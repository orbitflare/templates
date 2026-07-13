use async_trait::async_trait;
use orbitflare_sdk::jetstream::v2::{
    JetstreamClientBuilder, TransactionFilter, TransactionStream as SdkJetstreamStream,
};
use orbitflare_sdk::proto::jetstream::v2::{
    TransactionMessage, TxFilter, subscribe_transactions_response::Payload,
};
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

    fn build_filters(&self) -> Vec<TxFilter> {
        vec![
            TransactionFilter::new()
                .account_include(self.config.transactions.account_include.clone())
                .account_exclude(self.config.transactions.account_exclude.clone())
                .account_required(self.config.transactions.account_required.clone())
                .with_id("default"),
        ]
    }

    fn establish_stream(&self) -> Result<SdkJetstreamStream> {
        let client = JetstreamClientBuilder::new()
            .url(&self.url)
            .timeout_secs(self.config.timeout_secs)
            .keepalive_secs(self.config.tcp_keepalive_secs)
            .build()
            .map_err(|e| IndexerError::Connection(format!("jetstream build failed: {e}")))?;

        Ok(client.subscribe_transactions(self.build_filters()))
    }

    fn parse_transaction(tx: TransactionMessage) -> RawTransaction {
        let signature = bs58::encode(&tx.signature).into_string();

        let account_keys: Vec<String> = tx
            .account_keys
            .iter()
            .filter(|k| k.len() == 32)
            .map(|k| bs58::encode(k).into_string())
            .collect();

        RawTransaction {
            signature,
            slot: tx.slot,
            block_time: None,
            fee: None,
            success: true,
            err: None,
            num_instructions: tx.instructions.len() as u32,
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
        info!(url = %self.url, "connecting to jetstream (v2)");
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
            Some(Ok(resp)) => match resp.payload {
                Some(Payload::Transaction(ft)) => {
                    if let Some(tx) = ft.transaction {
                        debug!(slot = tx.slot, "jetstream tx");
                        Ok(Some(Self::parse_transaction(tx)))
                    } else {
                        Ok(None)
                    }
                }
                Some(Payload::FilterValidation(result)) => {
                    if !result.accepted {
                        warn!(
                            filter_id = %result.filter_id,
                            reason = %result.rejection_reason,
                            "jetstream rejected filter"
                        );
                    }
                    Ok(None)
                }
                Some(Payload::Heartbeat(_) | Payload::Pong(_)) => Ok(None),
                None => Ok(None),
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
