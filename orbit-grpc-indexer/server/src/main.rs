mod router;
mod state;
mod error;
mod filter;
mod response;
mod handler;
mod pagination;
mod health;

use std::sync::Arc;

use anyhow::Context;
use sqlx::postgres::PgListener;
use tokio::sync::broadcast;
use tracing::{error, info};

use indexer_config::load_config;
use indexer_db::pool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yml".to_string());

    let config = load_config(&std::path::PathBuf::from(&config_path))
        .context("failed to load config")?;
    let config = Arc::new(config);

    init_tracing(&config.logging);

    let db = pool::create_pool(&config.database_url, &config.database).await?;
    let health = Arc::new(health::SystemHealth::new(db.clone()));
    let (tx_broadcast, _) = broadcast::channel::<String>(1024);

    spawn_pg_listener(&config.database_url, tx_broadcast.clone());

    let state = state::AppState {
        db,
        config,
        health,
        tx_broadcast,
    };

    let router = router::build_router(state);
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!(addr = %addr, "indexer-api-server starting");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("indexer-api-server stopped");
    Ok(())
}

fn spawn_pg_listener(database_url: &str, tx: broadcast::Sender<String>) {
    let url = database_url.to_string();
    tokio::spawn(async move {
        loop {
            match PgListener::connect(&url).await {
                Ok(mut listener) => {
                    if let Err(e) = listener.listen("new_transaction").await {
                        error!(error = %e, "pg listen failed");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue;
                    }
                    info!("listening for new_transaction notifications");
                    loop {
                        match listener.recv().await {
                            Ok(notification) => {
                                let _ = tx.send(notification.payload().to_string());
                            }
                            Err(e) => {
                                error!(error = %e, "pg notification error, reconnecting");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "pg listener connect failed, retrying");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl+c handler");
    info!("ctrl+c received, shutting down");
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
