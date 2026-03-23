mod shutdown;
mod metrics;
mod pipeline;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tokio::sync::broadcast;
use tracing::info;

use indexer_config::load_config;
use indexer_db::pool;
use indexer_filter::CompositeFilter;
use indexer_ingest::jetstream::JetstreamStream;
use indexer_ingest::merge::DefaultEnricher;
use indexer_ingest::yellowstone::YellowstoneStream;

use crate::metrics::Metrics;
use crate::pipeline::Pipeline;
use crate::shutdown::ShutdownCoordinator;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yml".to_string());

    let config = load_config(&PathBuf::from(&config_path))
        .context("failed to load config")?;
    let config = Arc::new(config);

    init_tracing(&config.logging);
    info!(config_path = %config_path, "starting orbit-grpc-indexer");

    let db = pool::create_pool(&config.database_url, &config.database).await?;

    if config.database.run_migrations {
        let migrations_dir = PathBuf::from("migrations");
        if migrations_dir.exists() {
            pool::run_migrations(&db, &migrations_dir).await?;
        }
    }

    let (tx_broadcast, _) = broadcast::channel::<String>(1024);
    let shutdown = ShutdownCoordinator::new();
    let metrics = Arc::new(Metrics::new());

    let mut stream_handles = Vec::new();

    if config.jetstream.enabled {
        let stream = JetstreamStream::new(config.jetstream_url.clone(), config.jetstream.clone());
        let handle = Pipeline::spawn_stream(
            stream,
            filter_clone(&config.filters),
            DefaultEnricher::new(),
            indexer_db::sink::PostgresSink::new(db.clone(), config.batching.clone()),
            tx_broadcast.clone(),
            metrics.clone(),
            config.batching.clone(),
            shutdown.subscribe(),
        );
        stream_handles.push(handle);
    }

    if config.yellowstone.enabled {
        let last_slot = indexer_db::query::get_last_indexed_slot(&db).await?;
        if let Some(slot) = last_slot {
            info!(slot, "resuming yellowstone from last indexed slot");
        }
        let stream = YellowstoneStream::new(config.yellowstone_url.clone(), config.yellowstone.clone())
            .resume_from(last_slot);
        let handle = Pipeline::spawn_stream(
            stream,
            filter_clone(&config.filters),
            DefaultEnricher::new(),
            indexer_db::sink::PostgresSink::new(db.clone(), config.batching.clone()),
            tx_broadcast.clone(),
            metrics.clone(),
            config.batching.clone(),
            shutdown.subscribe(),
        );
        stream_handles.push(handle);
    }

    let metrics_clone = metrics.clone();
    let mut metrics_shutdown = shutdown.subscribe();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let snap = metrics_clone.snapshot();
                    info!(
                        jetstream_rx = snap.jetstream_received,
                        yellowstone_rx = snap.yellowstone_received,
                        indexed = snap.total_indexed,
                        filtered = snap.total_filtered,
                        errors = snap.total_errors,
                        "metrics"
                    );
                }
                _ = metrics_shutdown.changed() => break,
            }
        }
    });

    shutdown.wait_for_signal().await;
    info!("shutting down");

    for handle in stream_handles {
        let _ = handle.await;
    }

    info!("orbit-grpc-indexer stopped");
    Ok(())
}

fn init_tracing(config: &indexer_config::model::LoggingConfig) {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::prelude::*;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));

    if config.json {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
}

fn filter_clone(config: &indexer_config::model::FilterConfig) -> CompositeFilter {
    CompositeFilter::from_config(config)
}
