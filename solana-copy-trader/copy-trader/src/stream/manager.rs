use crate::config::AppConfig;
use crate::stream::filter::build_jetstream_filters;
use crate::types::{DetectionSource, RawInstruction, RawTransaction};
use orbitflare_sdk::jetstream::v2::JetstreamClientBuilder;
use orbitflare_sdk::proto::jetstream::v2::{
    subscribe_transactions_response::Payload, TransactionMessage,
};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub struct StreamManager {
    config: Arc<AppConfig>,
    tx: mpsc::Sender<RawTransaction>,
    shutdown_rx: watch::Receiver<bool>,
    slot_lag_gauge: prometheus::IntGauge,
    latest_seen_slot: u64,
}

impl StreamManager {
    pub fn new(
        config: Arc<AppConfig>,
        tx: mpsc::Sender<RawTransaction>,
        shutdown_rx: watch::Receiver<bool>,
        slot_lag_gauge: prometheus::IntGauge,
    ) -> Self {
        Self {
            config,
            tx,
            shutdown_rx,
            slot_lag_gauge,
            latest_seen_slot: 0,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!(url = %self.config.jetstream.url, "Connecting to Jetstream");

        let client = JetstreamClientBuilder::new()
            .url(&self.config.jetstream.url)
            .timeout_secs(self.config.jetstream.timeout_secs)
            .keepalive_secs(self.config.jetstream.tcp_keepalive_secs)
            .build()?;

        let filters = build_jetstream_filters(&self.config);

        tracing::info!(filter_count = filters.len(), "Subscribing to Jetstream (v2)");
        let mut stream = client.subscribe_transactions(filters);
        tracing::info!("Connected to Jetstream, streaming transactions");

        loop {
            tokio::select! {
                update = stream.next() => {
                    match update {
                        Some(Ok(resp)) => match resp.payload {
                            Some(Payload::Transaction(ft)) => {
                                if let Some(tx) = ft.transaction {
                                    let slot = tx.slot;
                                    if slot > self.latest_seen_slot {
                                        if self.latest_seen_slot > 0 {
                                            let lag = slot - self.latest_seen_slot;
                                            self.slot_lag_gauge.set(lag as i64);
                                        }
                                        self.latest_seen_slot = slot;
                                    }

                                    let raw = convert_jetstream_tx(tx);
                                    if self.tx.try_send(raw).is_err() {
                                        tracing::warn!("Transaction channel full, dropping Jetstream tx");
                                    }
                                }
                            }
                            Some(Payload::FilterValidation(result)) => {
                                if !result.accepted {
                                    tracing::warn!(
                                        filter_id = %result.filter_id,
                                        reason = %result.rejection_reason,
                                        "Jetstream rejected filter"
                                    );
                                }
                            }
                            Some(Payload::Heartbeat(_)) => tracing::trace!("Received heartbeat from Jetstream"),
                            Some(Payload::Pong(_)) => tracing::trace!("Received pong from Jetstream"),
                            None => {}
                        },
                        Some(Err(e)) => {
                            return Err(anyhow::anyhow!("Jetstream stream gave up after SDK retries: {}", e));
                        }
                        None => {
                            tracing::info!("Jetstream stream closed");
                            return Ok(());
                        }
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    tracing::info!("Jetstream received shutdown signal");
                    return Ok(());
                }
            }
        }
    }
}

fn convert_jetstream_tx(tx_info: TransactionMessage) -> RawTransaction {
    let signature = bs58::encode(&tx_info.signature).into_string();

    let account_keys: Vec<Pubkey> = tx_info
        .account_keys
        .iter()
        .filter_map(|bytes| {
            if bytes.len() == 32 {
                let mut array = [0u8; 32];
                array.copy_from_slice(bytes);
                Some(Pubkey::new_from_array(array))
            } else {
                None
            }
        })
        .collect();

    let instructions: Vec<RawInstruction> = tx_info
        .instructions
        .iter()
        .map(|ix| RawInstruction {
            program_id_index: ix.program_id_index,
            accounts: ix.accounts.to_vec(),
            data: ix.data.to_vec(),
        })
        .collect();

    RawTransaction {
        signature,
        slot: tx_info.slot,
        account_keys,
        instructions,
        inner_instructions: Vec::new(),
        source: DetectionSource::Jetstream,
    }
}
