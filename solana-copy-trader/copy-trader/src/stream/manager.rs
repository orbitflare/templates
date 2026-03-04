use crate::config::AppConfig;
use crate::stream::filter::build_subscription_filters;
use jetstream_protos::jetstream::{
    jetstream_client::JetstreamClient, subscribe_update::UpdateOneof, SubscribeRequest,
    SubscribeRequestPing, SubscribeUpdateTransactionInfo,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_stream::StreamExt;
use tonic::transport::Channel;

pub struct StreamManager {
    config: Arc<AppConfig>,
    tx: mpsc::Sender<SubscribeUpdateTransactionInfo>,
    shutdown_rx: watch::Receiver<bool>,
    reconnect_counter: prometheus::IntCounter,
    slot_lag_gauge: prometheus::IntGauge,
    latest_seen_slot: u64,
}

impl StreamManager {
    pub fn new(
        config: Arc<AppConfig>,
        tx: mpsc::Sender<SubscribeUpdateTransactionInfo>,
        shutdown_rx: watch::Receiver<bool>,
        reconnect_counter: prometheus::IntCounter,
        slot_lag_gauge: prometheus::IntGauge,
    ) -> Self {
        Self {
            config,
            tx,
            shutdown_rx,
            reconnect_counter,
            slot_lag_gauge,
            latest_seen_slot: 0,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut delay_ms = self.config.jetstream.reconnect.initial_delay_ms;

        loop {
            if *self.shutdown_rx.borrow() {
                tracing::info!("StreamManager shutting down");
                return Ok(());
            }

            match self.connect_and_stream().await {
                Ok(()) => {
                    tracing::info!("Stream ended normally");
                    return Ok(());
                }
                Err(e) => {
                    tracing::error!(error = %e, delay_ms, "Stream disconnected, reconnecting...");
                    self.reconnect_counter.inc();

                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                        _ = self.shutdown_rx.changed() => {
                            tracing::info!("StreamManager shutting down during reconnect backoff");
                            return Ok(());
                        }
                    }

                    delay_ms = (delay_ms as f64 * self.config.jetstream.reconnect.multiplier) as u64;
                    delay_ms = delay_ms.min(self.config.jetstream.reconnect.max_delay_ms);
                }
            }
        }
    }

    async fn connect_and_stream(&mut self) -> anyhow::Result<()> {
        tracing::info!(url = %self.config.jetstream.url, "Connecting to Jetstream");

        let channel = Channel::from_shared(self.config.jetstream.url.clone())?
            .timeout(Duration::from_secs(self.config.jetstream.timeout_secs))
            .tcp_keepalive(Some(Duration::from_secs(
                self.config.jetstream.tcp_keepalive_secs,
            )))
            .connect()
            .await?;

        let mut client = JetstreamClient::new(channel);
        let filters = build_subscription_filters(&self.config);

        tracing::info!(filter_count = filters.len(), "Subscribing to Jetstream");

        let request = SubscribeRequest {
            transactions: filters,
            accounts: HashMap::new(),
            ping: Some(SubscribeRequestPing { id: 1 }),
        };

        let outbound = tokio_stream::iter(vec![request]);
        let response = client.subscribe(outbound).await?;
        let mut inbound = response.into_inner();

        tracing::info!("Connected to Jetstream, streaming transactions");

        loop {
            tokio::select! {
                msg = inbound.next() => {
                    match msg {
                        Some(Ok(update)) => {
                            match update.update_oneof {
                                Some(UpdateOneof::Transaction(tx_update)) => {
                                    let slot = tx_update.slot;
                                    if slot > self.latest_seen_slot {
                                        if self.latest_seen_slot > 0 {
                                            let lag = slot - self.latest_seen_slot;
                                            self.slot_lag_gauge.set(lag as i64);
                                        }
                                        self.latest_seen_slot = slot;
                                    }

                                    if let Some(tx_info) = tx_update.transaction {
                                        if self.tx.try_send(tx_info).is_err() {
                                            tracing::warn!("Transaction channel full, dropping transaction");
                                        }
                                    }
                                }
                                Some(UpdateOneof::Ping(_)) => {
                                    tracing::trace!("Received ping from Jetstream");
                                }
                                Some(UpdateOneof::Pong(_)) => {
                                    tracing::trace!("Received pong from Jetstream");
                                }
                                _ => {}
                            }
                        }
                        Some(Err(e)) => {
                            return Err(anyhow::anyhow!("Stream error: {}", e));
                        }
                        None => {
                            return Err(anyhow::anyhow!("Stream ended unexpectedly"));
                        }
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    tracing::info!("StreamManager received shutdown signal");
                    return Ok(());
                }
            }
        }
    }
}
