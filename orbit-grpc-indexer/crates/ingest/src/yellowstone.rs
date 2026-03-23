use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tracing::{debug, error, info, warn};

use indexer_config::model::YellowstoneConfig;
use indexer_core::error::{IndexerError, Result};
use indexer_core::stream::TransactionStream;
use indexer_core::types::{InnerInstruction, RawTransaction, StreamSource};
use indexer_proto::geyser::{
    geyser_client::GeyserClient,
    subscribe_update::UpdateOneof,
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterSlots,
    SubscribeRequestFilterTransactions, SubscribeRequestPing,
};

use crate::backoff::Backoff;

type GrpcStream = tonic::Streaming<indexer_proto::geyser::SubscribeUpdate>;

pub struct YellowstoneStream {
    url: String,
    config: YellowstoneConfig,
    stream: Option<GrpcStream>,
    backoff: Backoff,
    last_slot: Option<u64>,
}

impl YellowstoneStream {
    pub fn new(url: String, config: YellowstoneConfig) -> Self {
        let backoff = Backoff::from_config(&config.reconnect);
        Self {
            url,
            config,
            stream: None,
            backoff,
            last_slot: None,
        }
    }

    pub fn resume_from(mut self, slot: Option<u64>) -> Self {
        self.last_slot = slot;
        self
    }

    fn parse_commitment(s: &str) -> CommitmentLevel {
        match s {
            "processed" => CommitmentLevel::Processed,
            "finalized" => CommitmentLevel::Finalized,
            _ => CommitmentLevel::Confirmed,
        }
    }

    async fn create_channel(&self, url: &str) -> Result<Channel> {
        Channel::from_shared(url.to_string())
            .map_err(|e| IndexerError::Connection(format!("invalid endpoint: {e}")))?
            .timeout(Duration::from_secs(self.config.timeout_secs))
            .tcp_keepalive(Some(Duration::from_secs(self.config.tcp_keepalive_secs)))
            .connect_timeout(Duration::from_secs(self.config.timeout_secs))
            .connect()
            .await
            .map_err(|e| IndexerError::Connection(format!("yellowstone connect failed: {e}")))
    }

    fn build_subscribe_request(&self) -> SubscribeRequest {
        let mut transactions = HashMap::new();
        transactions.insert(
            "default".to_string(),
            SubscribeRequestFilterTransactions {
                vote: self.config.transactions.vote,
                failed: self.config.transactions.failed,
                signature: None,
                account_include: self.config.transactions.account_include.clone(),
                account_exclude: self.config.transactions.account_exclude.clone(),
                account_required: self.config.transactions.account_required.clone(),
            },
        );

        let commitment = Self::parse_commitment(&self.config.commitment);

        SubscribeRequest {
            accounts: HashMap::new(),
            slots: if self.config.slots.enabled {
                HashMap::from([(
                    "default".to_string(),
                    SubscribeRequestFilterSlots {
                        filter_by_commitment: self.config.slots.filter_by_commitment,
                        interslot_updates: None,
                    },
                )])
            } else {
                HashMap::new()
            },
            transactions,
            transactions_status: HashMap::new(),
            blocks: HashMap::new(),
            blocks_meta: HashMap::new(),
            entry: HashMap::new(),
            commitment: Some(commitment.into()),
            accounts_data_slice: vec![],
            ping: Some(SubscribeRequestPing { id: 1 }),
            from_slot: self.last_slot,
        }
    }

    async fn establish_stream(&self) -> Result<GrpcStream> {
        let channel = self.create_channel(&self.url).await?;
        let mut client = GeyserClient::new(channel);

        let request = self.build_subscribe_request();
        debug!(
            tx_filters = ?request.transactions.keys().collect::<Vec<_>>(),
            slot_filters = ?request.slots.keys().collect::<Vec<_>>(),
            commitment = ?request.commitment,
            "yellowstone subscribe request"
        );
        let outbound = tokio_stream::iter(vec![request]);

        let response = client
            .subscribe(outbound)
            .await
            .map_err(|e| IndexerError::Stream(format!("yellowstone subscribe failed: {e}")))?;

        Ok(response.into_inner())
    }

    fn parse_transaction(
        tx_info: indexer_proto::geyser::SubscribeUpdateTransactionInfo,
        slot: u64,
    ) -> Option<RawTransaction> {
        let signature = bs58::encode(&tx_info.signature).into_string();

        let meta = tx_info.meta.as_ref();
        let transaction = tx_info.transaction.as_ref()?;
        let message = transaction.message.as_ref()?;

        let account_keys: Vec<String> = message
            .account_keys
            .iter()
            .filter(|k| k.len() == 32)
            .map(|k| bs58::encode(k).into_string())
            .collect();

        let fee = meta.map(|m| m.fee);

        let success = meta
            .map(|m| m.err.is_none() || m.err.as_ref().is_some_and(|e| e.err.is_empty()))
            .unwrap_or(true);

        let err = meta
            .and_then(|m| m.err.as_ref())
            .filter(|e| !e.err.is_empty())
            .map(|e| serde_json::json!({ "err": bs58::encode(&e.err).into_string() }));

        let log_messages = meta
            .map(|m| m.log_messages.clone())
            .unwrap_or_default();

        let inner_instructions = meta
            .map(|m| {
                let keys = &account_keys;
                m.inner_instructions
                    .iter()
                    .flat_map(|ii| {
                        let idx = ii.index;
                        ii.instructions.iter().map(move |instr| {
                            let program_id = keys
                                .get(instr.program_id_index as usize)
                                .cloned()
                                .unwrap_or_default();

                            let instr_accounts: Vec<String> = instr
                                .accounts
                                .iter()
                                .filter_map(|&i| keys.get(i as usize).cloned())
                                .collect();

                            InnerInstruction {
                                instruction_index: idx,
                                depth: instr.stack_height.unwrap_or(1),
                                program_id,
                                accounts: instr_accounts,
                                data: bs58::encode(&instr.data).into_string(),
                            }
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let num_instructions = message.instructions.len() as u32;

        Some(RawTransaction {
            signature,
            slot,
            block_time: None,
            fee,
            success,
            err,
            num_instructions,
            account_keys,
            log_messages,
            inner_instructions,
            raw: None,
            source: StreamSource::Yellowstone,
        })
    }
}

#[async_trait]
impl TransactionStream for YellowstoneStream {
    async fn connect(&mut self) -> Result<()> {
        info!(url = %self.url, "connecting to yellowstone");
        let stream = self.establish_stream().await?;
        self.stream = Some(stream);
        self.backoff.reset();
        info!("yellowstone stream connected");
        Ok(())
    }

    async fn next(&mut self) -> Result<Option<RawTransaction>> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| IndexerError::Stream("yellowstone not connected".into()))?;

        match stream.next().await {
            Some(Ok(update)) => match update.update_oneof {
                Some(UpdateOneof::Transaction(tx_update)) => {
                    let slot = tx_update.slot;
                    self.last_slot = Some(slot);
                    if let Some(tx_info) = tx_update.transaction {
                        debug!(slot, "yellowstone tx");
                        Ok(Self::parse_transaction(tx_info, slot))
                    } else {
                        Ok(None)
                    }
                }
                Some(UpdateOneof::Ping(_) | UpdateOneof::Pong(_)) => {
                    debug!("yellowstone ping/pong");
                    Ok(None)
                }
                Some(other) => {
                    debug!("yellowstone non-tx update: {:?}", std::mem::discriminant(&other));
                    Ok(None)
                }
                None => {
                    debug!("yellowstone empty update");
                    Ok(None)
                }
            },
            Some(Err(e)) => {
                error!(error = %e, "yellowstone stream error");
                Err(IndexerError::Stream(format!("yellowstone: {e}")))
            }
            None => {
                warn!("yellowstone stream ended");
                Err(IndexerError::Stream("yellowstone stream ended".into()))
            }
        }
    }

    async fn reconnect(&mut self) -> Result<()> {
        self.stream = None;

        let delay = self.backoff.next_delay().ok_or_else(|| {
            IndexerError::Connection("yellowstone max retries exhausted".into())
        })?;

        warn!(
            attempt = self.backoff.attempt(),
            delay_ms = delay.as_millis() as u64,
            "reconnecting to yellowstone"
        );

        tokio::time::sleep(delay).await;
        self.connect().await
    }

    fn source(&self) -> StreamSource {
        StreamSource::Yellowstone
    }
}
