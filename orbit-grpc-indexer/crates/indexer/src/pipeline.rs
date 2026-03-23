use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use indexer_config::model::BatchConfig;
use indexer_core::enricher::TransactionEnricher;
use indexer_core::filter::TransactionFilter;
use indexer_core::sink::TransactionSink;
use indexer_core::stream::TransactionStream;
use indexer_core::types::{ProcessedTransaction, StreamSource};

use crate::metrics::Metrics;

pub struct Pipeline;

impl Pipeline {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_stream<S, F, E, K>(
        mut stream: S,
        filter: F,
        enricher: E,
        sink: K,
        tx_broadcast: broadcast::Sender<String>,
        metrics: Arc<Metrics>,
        batch_config: BatchConfig,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> JoinHandle<()>
    where
        S: TransactionStream + 'static,
        F: TransactionFilter + 'static,
        E: TransactionEnricher + 'static,
        K: TransactionSink + 'static,
    {
        tokio::spawn(async move {
            let source = stream.source();

            loop {
                match stream.connect().await {
                    Ok(()) => {
                        set_connected(&metrics, source, true);
                        break;
                    }
                    Err(e) => {
                        error!(source = %source, error = %e, "initial connection failed");
                        match stream.reconnect().await {
                            Ok(()) => {
                                set_connected(&metrics, source, true);
                                break;
                            }
                            Err(e) => {
                                error!(source = %source, error = %e, "reconnect failed, retrying");
                                continue;
                            }
                        }
                    }
                }
            }

            let mut batch: Vec<ProcessedTransaction> = Vec::with_capacity(batch_config.size);
            let flush_interval = Duration::from_millis(batch_config.flush_interval_ms);
            let mut flush_timer = tokio::time::interval(flush_interval);

            info!(source = %source, "pipeline started");

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        info!(source = %source, "pipeline shutting down");
                        if !batch.is_empty() {
                            if let Err(e) = sink.write_batch(&batch).await {
                                error!(error = %e, "final flush failed");
                            }
                        }
                        set_connected(&metrics, source, false);
                        break;
                    }

                    _ = flush_timer.tick() => {
                        if !batch.is_empty() {
                            debug!(source = %source, count = batch.len(), "flushing batch (timer)");
                            if let Err(e) = sink.write_batch(&batch).await {
                                error!(error = %e, "batch flush failed");
                                metrics.total_errors.fetch_add(1, Ordering::Relaxed);
                            } else {
                                metrics.total_indexed.fetch_add(batch.len() as u64, Ordering::Relaxed);
                            }
                            batch.clear();
                        }
                    }

                    result = stream.next() => {
                        match result {
                            Ok(Some(raw_tx)) => {
                                match source {
                                    StreamSource::Jetstream => {
                                        metrics.jetstream_received.fetch_add(1, Ordering::Relaxed);
                                    }
                                    StreamSource::Yellowstone => {
                                        metrics.yellowstone_received.fetch_add(1, Ordering::Relaxed);
                                    }
                                }

                                if !filter.filter(&raw_tx) {
                                    metrics.total_filtered.fetch_add(1, Ordering::Relaxed);
                                    continue;
                                }

                                let processed = match enricher.enrich(raw_tx).await {
                                    Ok(p) => p,
                                    Err(e) => {
                                        error!(error = %e, "enrichment failed");
                                        metrics.total_errors.fetch_add(1, Ordering::Relaxed);
                                        continue;
                                    }
                                };

                                if let Ok(json) = serde_json::to_string(&serde_json::json!({
                                    "signature": &processed.signature,
                                    "slot": processed.slot,
                                    "source": processed.source.to_string(),
                                    "success": processed.success,
                                    "has_cpi_data": processed.has_cpi_data,
                                })) {
                                    let _ = tx_broadcast.send(json);
                                }

                                batch.push(processed);

                                if batch.len() >= batch_config.size {
                                    debug!(source = %source, count = batch.len(), "flushing batch (full)");
                                    if let Err(e) = sink.write_batch(&batch).await {
                                        error!(error = %e, "batch flush failed");
                                        metrics.total_errors.fetch_add(1, Ordering::Relaxed);
                                    } else {
                                        metrics.total_indexed.fetch_add(batch.len() as u64, Ordering::Relaxed);
                                    }
                                    batch.clear();
                                }
                            }
                            Ok(None) => continue,
                            Err(e) => {
                                error!(source = %source, error = %e, "stream error");
                                set_connected(&metrics, source, false);
                                metrics.total_errors.fetch_add(1, Ordering::Relaxed);

                                if !batch.is_empty() {
                                    let _ = sink.write_batch(&batch).await;
                                    batch.clear();
                                }

                                match stream.reconnect().await {
                                    Ok(()) => {
                                        set_connected(&metrics, source, true);
                                        info!(source = %source, "reconnected");
                                    }
                                    Err(e) => {
                                        error!(source = %source, error = %e, "reconnect failed, exiting");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            info!(source = %source, "pipeline stopped");
        })
    }
}

fn set_connected(metrics: &Metrics, source: StreamSource, connected: bool) {
    let val = if connected { 1 } else { 0 };
    match source {
        StreamSource::Jetstream => metrics.jetstream_connected.store(val, Ordering::Relaxed),
        StreamSource::Yellowstone => metrics.yellowstone_connected.store(val, Ordering::Relaxed),
    }
}
