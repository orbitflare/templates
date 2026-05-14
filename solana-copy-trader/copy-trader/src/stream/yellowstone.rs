use crate::config::AppConfig;
use crate::stream::filter::build_yellowstone_filters;
use crate::types::{DetectionSource, InnerInstructionSet, RawInstruction, RawTransaction};
use orbitflare_sdk::proto::geyser::{
    subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest, SubscribeRequestPing,
    SubscribeUpdateTransactionInfo,
};
use orbitflare_sdk::GeyserClientBuilder;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub struct YellowstoneManager {
    config: Arc<AppConfig>,
    tx: mpsc::Sender<RawTransaction>,
    shutdown_rx: watch::Receiver<bool>,
}

impl YellowstoneManager {
    pub fn new(
        config: Arc<AppConfig>,
        tx: mpsc::Sender<RawTransaction>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            config,
            tx,
            shutdown_rx,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let ys_config = &self.config.yellowstone;
        tracing::info!(url = %ys_config.url, "Connecting to Yellowstone gRPC");

        let client = GeyserClientBuilder::new()
            .url(&ys_config.url)
            .timeout_secs(ys_config.timeout_secs)
            .keepalive_secs(ys_config.tcp_keepalive_secs)
            .build()?;

        let tx_filters = build_yellowstone_filters(&self.config);

        tracing::info!(
            filter_count = tx_filters.len(),
            "Subscribing to Yellowstone gRPC"
        );

        let commitment = match self.config.rpc.commitment.as_str() {
            "finalized" => CommitmentLevel::Finalized as i32,
            "confirmed" => CommitmentLevel::Confirmed as i32,
            _ => CommitmentLevel::Processed as i32,
        };

        let request = SubscribeRequest {
            accounts: HashMap::new(),
            slots: HashMap::new(),
            transactions: tx_filters,
            transactions_status: HashMap::new(),
            blocks: HashMap::new(),
            blocks_meta: HashMap::new(),
            entry: HashMap::new(),
            commitment: Some(commitment),
            accounts_data_slice: vec![],
            ping: Some(SubscribeRequestPing { id: 1 }),
            from_slot: None,
        };

        let mut stream = client.subscribe(request);
        tracing::info!("Connected to Yellowstone, streaming transactions with inner instructions");

        loop {
            tokio::select! {
                update = stream.next() => {
                    match update {
                        Some(Ok(update)) => match update.update_oneof {
                            Some(UpdateOneof::Transaction(tx_update)) => {
                                let slot = tx_update.slot;
                                if let Some(tx_info) = tx_update.transaction {
                                    let mut raw = convert_yellowstone_tx(tx_info);
                                    raw.slot = slot;
                                    if self.tx.try_send(raw).is_err() {
                                        tracing::warn!("Transaction channel full, dropping Yellowstone tx");
                                    }
                                }
                            }
                            Some(UpdateOneof::Ping(_)) => tracing::trace!("Received ping from Yellowstone"),
                            Some(UpdateOneof::Pong(_)) => tracing::trace!("Received pong from Yellowstone"),
                            _ => {}
                        },
                        Some(Err(e)) => {
                            return Err(anyhow::anyhow!("Yellowstone stream gave up after SDK retries: {}", e));
                        }
                        None => {
                            tracing::info!("Yellowstone stream closed");
                            return Ok(());
                        }
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    tracing::info!("Yellowstone received shutdown signal");
                    return Ok(());
                }
            }
        }
    }
}

fn convert_yellowstone_tx(tx_info: SubscribeUpdateTransactionInfo) -> RawTransaction {
    let signature = bs58::encode(&tx_info.signature).into_string();

    let tx_msg = tx_info
        .transaction
        .as_ref()
        .and_then(|t| t.message.as_ref());

    let mut account_keys: Vec<Pubkey> = Vec::new();

    if let Some(msg) = tx_msg {
        for bytes in &msg.account_keys {
            if bytes.len() == 32 {
                let mut array = [0u8; 32];
                array.copy_from_slice(bytes);
                account_keys.push(Pubkey::new_from_array(array));
            }
        }
    }

    if let Some(meta) = &tx_info.meta {
        for bytes in &meta.loaded_writable_addresses {
            if bytes.len() == 32 {
                let mut array = [0u8; 32];
                array.copy_from_slice(bytes);
                account_keys.push(Pubkey::new_from_array(array));
            }
        }
        for bytes in &meta.loaded_readonly_addresses {
            if bytes.len() == 32 {
                let mut array = [0u8; 32];
                array.copy_from_slice(bytes);
                account_keys.push(Pubkey::new_from_array(array));
            }
        }
    }

    let instructions: Vec<RawInstruction> = tx_msg
        .map(|msg| {
            msg.instructions
                .iter()
                .map(|ix| RawInstruction {
                    program_id_index: ix.program_id_index,
                    accounts: ix.accounts.to_vec(),
                    data: ix.data.to_vec(),
                })
                .collect()
        })
        .unwrap_or_default();

    let inner_instructions: Vec<InnerInstructionSet> = tx_info
        .meta
        .as_ref()
        .map(|meta| {
            meta.inner_instructions
                .iter()
                .map(|inner_set| InnerInstructionSet {
                    index: inner_set.index,
                    instructions: inner_set
                        .instructions
                        .iter()
                        .map(|ix| RawInstruction {
                            program_id_index: ix.program_id_index,
                            accounts: ix.accounts.to_vec(),
                            data: ix.data.to_vec(),
                        })
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();

    RawTransaction {
        signature,
        slot: 0,
        account_keys,
        instructions,
        inner_instructions,
        source: DetectionSource::Yellowstone,
    }
}
