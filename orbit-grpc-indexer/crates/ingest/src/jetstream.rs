use std::collections::HashMap;

use async_trait::async_trait;
use orbitflare_sdk::proto::jetstream::{
    SubscribeRequest, SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions,
    SubscribeRequestPing, SubscribeUpdateTransactionInfo, subscribe_update::UpdateOneof,
};
use orbitflare_sdk::{JetstreamClientBuilder, JetstreamStream as SdkJetstreamStream};
use tracing::{debug, error, info, warn};

use indexer_config::model::JetstreamConfig;
use indexer_core::error::{IndexerError, Result};
use indexer_core::stream::TransactionStream;
use indexer_core::types::{RawTransaction, StreamSource};

use crate::backoff::Backoff;

pub struct JetstreamStream {
    url: String,
    config: JetstreamConfig,
    stream: Option<SdkJetstreamStream>,
    backoff: Backoff,
}

impl JetstreamStream {
    pub fn new(url: String, config: JetstreamConfig) -> Self {
        let backoff = Backoff::from_config(&config.reconnect);
        Self {
            url,
            config,
            stream: None,
            backoff,
        }
    }

    fn build_subscribe_request(&self) -> SubscribeRequest {
        let mut transactions = HashMap::new();
        transactions.insert(
            "default".to_string(),
            SubscribeRequestFilterTransactions {
                account_include: self.config.transactions.account_include.clone(),
                account_exclude: self.config.transactions.account_exclude.clone(),
                account_required: self.config.transactions.account_required.clone(),
            },
        );

        let mut accounts = HashMap::new();
        if !self.config.accounts.account.is_empty() || !self.config.accounts.owner.is_empty() {
            accounts.insert(
                "default".to_string(),
                SubscribeRequestFilterAccounts {
                    account: self.config.accounts.account.clone(),
                    owner: self.config.accounts.owner.clone(),
                    filters: vec![],
                },
            );
        }

        SubscribeRequest {
            transactions,
            accounts,
            ping: Some(SubscribeRequestPing { id: 1 }),
        }
    }

    fn establish_stream(&self) -> Result<SdkJetstreamStream> {
        let client = JetstreamClientBuilder::new()
            .url(&self.url)
            .timeout_secs(self.config.timeout_secs)
            .keepalive_secs(self.config.tcp_keepalive_secs)
            .build()
            .map_err(|e| IndexerError::Connection(format!("jetstream build failed: {e}")))?;

        Ok(client.subscribe(self.build_subscribe_request()))
    }

    fn parse_transaction(tx_info: SubscribeUpdateTransactionInfo, slot: u64) -> RawTransaction {
        let signature = bs58::encode(&tx_info.signature).into_string();

        let account_keys: Vec<String> = tx_info
            .account_keys
            .iter()
            .filter(|k| k.len() == 32)
            .map(|k| bs58::encode(k).into_string())
            .collect();

        RawTransaction {
            signature,
            slot,
            block_time: None,
            fee: None,
            success: true,
            err: None,
            num_instructions: tx_info.instructions.len() as u32,
            account_keys,
            log_messages: vec![],
            inner_instructions: vec![],
            raw: None,
            source: StreamSource::Jetstream,
        }
    }
}

#[async_trait]
impl TransactionStream for JetstreamStream {
    async fn connect(&mut self) -> Result<()> {
        info!(url = %self.url, "connecting to jetstream");
        self.stream = Some(self.establish_stream()?);
        self.backoff.reset();
        info!("jetstream stream connected");
        Ok(())
    }

    async fn next(&mut self) -> Result<Option<RawTransaction>> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| IndexerError::Stream("jetstream not connected".into()))?;

        match stream.next().await {
            Some(Ok(update)) => match update.update_oneof {
                Some(UpdateOneof::Transaction(tx_update)) => {
                    let slot = tx_update.slot;
                    if let Some(tx_info) = tx_update.transaction {
                        debug!(slot, "jetstream tx");
                        Ok(Some(Self::parse_transaction(tx_info, slot)))
                    } else {
                        Ok(None)
                    }
                }
                Some(UpdateOneof::Ping(_) | UpdateOneof::Pong(_)) => Ok(None),
                _ => Ok(None),
            },
            Some(Err(e)) => {
                error!(error = %e, "jetstream stream error");
                Err(IndexerError::Stream(format!("jetstream: {e}")))
            }
            None => {
                warn!("jetstream stream ended");
                Err(IndexerError::Stream("jetstream stream ended".into()))
            }
        }
    }

    async fn reconnect(&mut self) -> Result<()> {
        self.stream = None;

        let delay = self
            .backoff
            .next_delay()
            .ok_or_else(|| IndexerError::Connection("jetstream max retries exhausted".into()))?;

        warn!(
            attempt = self.backoff.attempt(),
            delay_ms = delay.as_millis() as u64,
            "reconnecting to jetstream"
        );

        tokio::time::sleep(delay).await;
        self.connect().await
    }

    fn source(&self) -> StreamSource {
        StreamSource::Jetstream
    }
}
