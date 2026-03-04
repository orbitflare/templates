use clap::Parser;
use copy_trader::config::{self, Cli, LogFormat};
use copy_trader::decoder::DecoderPipeline;
use copy_trader::execution::engine::ExecutionEngine;
use copy_trader::output::journal::TradeJournal;
use copy_trader::output::metrics::{self, Metrics};
use copy_trader::output::telegram::TelegramNotifier;
use copy_trader::state::redis::RedisClient;
use copy_trader::stream::manager::StreamManager;
use copy_trader::types::{TradeIntent, TradeRecord};
use solana_sdk::signature::{Keypair, Signer};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = match cli.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    let log_format = std::fs::read_to_string(&cli.config)
        .ok()
        .and_then(|raw| {
            let expanded = shellexpand::env(&raw).ok()?;
            let config: serde_yml::Value = serde_yml::from_str(&expanded).ok()?;
            config
                .get("logging")
                .and_then(|l| l.get("format"))
                .and_then(|f| f.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "json".to_string());

    let parsed_format = match log_format.as_str() {
        "pretty" => LogFormat::Pretty,
        _ => LogFormat::Json,
    };

    match parsed_format {
        LogFormat::Pretty => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(true)
                .pretty()
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(true)
                .json()
                .init();
        }
    }

    tracing::info!("Solana Copy Trader starting");

    let app_config = config::load_config(&cli.config)?;
    let app_config = config::apply_cli_overrides(app_config, &cli);

    if cli.validate {
        config::validate_config(&app_config)?;
        tracing::info!("Config is valid");
        return Ok(());
    }

    if cli.migrate {
        if !app_config.journal.enabled || app_config.journal.database_url.is_empty() {
            anyhow::bail!("journal.database_url must be set for --migrate");
        }
        TradeJournal::run_migrations(&app_config.journal.database_url).await?;
        return Ok(());
    }

    config::validate_config(&app_config)?;

    let config = Arc::new(app_config);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Received SIGINT/SIGTERM, initiating graceful shutdown");
        let _ = shutdown_tx.send(true);
    });

    let metrics = Arc::new(Metrics::new());
    let redis = Arc::new(RedisClient::connect(config.clone()).await?);

    let keypair = if !config.execution.dry_run {
        let keypair_path = std::env::var("TRADER_KEYPAIR_PATH")
            .unwrap_or_else(|_| "trader.json".to_string());
        let keypair_data = std::fs::read_to_string(&keypair_path)
            .map_err(|e| anyhow::anyhow!("Failed to read keypair from {}: {}", keypair_path, e))?;
        let keypair_bytes: Vec<u8> = serde_json::from_str(&keypair_data)?;
        let kp = Keypair::try_from(keypair_bytes.as_slice())
            .map_err(|e| anyhow::anyhow!("Invalid keypair: {}", e))?;
        tracing::info!(pubkey = %kp.pubkey(), "Trader keypair loaded");
        Some(Arc::new(kp))
    } else {
        tracing::info!("[DRY RUN] No keypair needed — generating throwaway for display");
        let kp = Keypair::new();
        tracing::info!(pubkey = %kp.pubkey(), "[DRY RUN] Ephemeral wallet");
        Some(Arc::new(kp))
    };

    let (tx_sender, tx_receiver) = mpsc::channel(config.jetstream.channel_buffer_size);
    let (intent_sender, intent_receiver) = mpsc::channel::<TradeIntent>(1000);
    let (record_sender, record_receiver) = mpsc::channel::<TradeRecord>(1000);

    let mut stream_mgr = StreamManager::new(
        config.clone(),
        tx_sender,
        shutdown_rx.clone(),
        metrics.stream_reconnects.clone(),
        metrics.stream_lag_slots.clone(),
    );
    let stream_handle = tokio::spawn(async move {
        if let Err(e) = stream_mgr.run().await {
            tracing::error!(error = %e, "StreamManager exited with error");
        }
    });

    let decoder_pipeline = DecoderPipeline::from_config(config.clone());
    let decoder_redis = redis.clone();
    let mut decoder_shutdown = shutdown_rx.clone();
    let decoder_handle = tokio::spawn(async move {
        let mut rx = tx_receiver;
        loop {
            tokio::select! {
                Some(tx_info) = rx.recv() => {
                    let sig = bs58::encode(&tx_info.signature).into_string();

                    match decoder_redis.check_dedup(&sig).await {
                        Ok(true) => {
                            tracing::trace!(sig, "Duplicate transaction, skipping");
                            continue;
                        }
                        Ok(false) => {}
                        Err(e) => {
                            tracing::warn!(error = %e, "Redis dedup check failed, processing anyway");
                        }
                    }

                    let intents = decoder_pipeline.decode_transaction(&tx_info);
                    for intent in intents {
                        if intent_sender.send(intent).await.is_err() {
                            tracing::warn!("Intent channel closed");
                            return;
                        }
                    }
                }
                _ = decoder_shutdown.changed() => {
                    tracing::info!("Decoder loop shutting down");
                    return;
                }
            }
        }
    });

    let engine = ExecutionEngine::new(
        config.clone(),
        redis.clone(),
        keypair,
        metrics.clone(),
        intent_receiver,
        record_sender.clone(),
        shutdown_rx.clone(),
    );
    let engine_handle = tokio::spawn(async move {
        engine.run().await;
    });

    let journal_handle = if config.journal.enabled && !config.journal.database_url.is_empty() {
        let (journal_tx, journal_rx) = mpsc::channel::<TradeRecord>(1000);
        let journal = TradeJournal::new(config.clone(), journal_rx, shutdown_rx.clone()).await?;

        let handle = tokio::spawn(async move {
            journal.run().await;
        });

        Some((handle, journal_tx))
    } else {
        tracing::info!("Trade journal disabled");
        None
    };

    let tee_redis = redis.clone();
    let journal_tx = journal_handle.as_ref().map(|(_, tx)| tx.clone());
    let mut tee_shutdown = shutdown_rx.clone();
    let mut record_rx = record_receiver;

    let (telegram_tx, telegram_rx) = if config.notifications.telegram.enabled {
        let (tx, rx) = mpsc::channel::<TradeRecord>(100);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    if let Some(rx) = telegram_rx {
        let notifier = TelegramNotifier::new(config.clone());
        let tg_shutdown = shutdown_rx.clone();
        tokio::spawn(async move {
            notifier.run(rx, tg_shutdown).await;
        });
    }

    let tee_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(record) = record_rx.recv() => {
                    if let Some(ref jtx) = journal_tx {
                        let _ = jtx.send(record.clone()).await;
                    }
                    if let Some(ref ttx) = telegram_tx {
                        let _ = ttx.send(record.clone()).await;
                    }
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "target_wallet": record.target_wallet,
                        "status": record.status.to_string(),
                        "dex": record.dex.to_string(),
                        "direction": record.direction.to_string(),
                        "output_mint": record.output_mint,
                        "our_amount_sol": record.our_amount_sol,
                        "dry_run": record.dry_run,
                    })) {
                        let _ = tee_redis.publish_trade_event(&json).await;
                    }
                }
                _ = tee_shutdown.changed() => {
                    tracing::info!("Record tee shutting down");
                    return;
                }
            }
        }
    });

    if config.metrics.enabled {
        let listen = config.metrics.listen.clone();
        let m = metrics.clone();
        let sr = shutdown_rx.clone();
        tokio::spawn(async move {
            if let Err(e) = metrics::serve_metrics(listen, m, sr).await {
                tracing::error!(error = %e, "Metrics server error");
            }
        });
    }

    tracing::info!(
        dry_run = config.execution.dry_run,
        targets = config.targets.len(),
        "Copy trader running — press Ctrl+C to stop"
    );

    let _ = tokio::join!(stream_handle, decoder_handle, engine_handle, tee_handle);

    if let Some((jh, _)) = journal_handle {
        let _ = jh.await;
    }

    tracing::info!("Solana Copy Trader stopped");
    Ok(())
}
