use async_trait::async_trait;

use indexer_core::enricher::TransactionEnricher;
use indexer_core::error::Result;
use indexer_core::types::{ProcessedTransaction, RawTransaction, StreamSource};

#[derive(Default)]
pub struct DefaultEnricher;

impl DefaultEnricher {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TransactionEnricher for DefaultEnricher {
    async fn enrich(&self, tx: RawTransaction) -> Result<ProcessedTransaction> {
        let has_cpi_data = match tx.source {
            StreamSource::Yellowstone => !tx.inner_instructions.is_empty(),
            StreamSource::Jetstream => false,
        };

        Ok(ProcessedTransaction {
            signature: tx.signature,
            slot: tx.slot,
            block_time: tx.block_time,
            fee: tx.fee,
            success: tx.success,
            err: tx.err,
            num_instructions: tx.num_instructions,
            account_keys: tx.account_keys,
            log_messages: tx.log_messages,
            inner_instructions: tx.inner_instructions,
            has_cpi_data,
            source: tx.source,
            raw: tx.raw,
        })
    }

    fn name(&self) -> &str {
        "default"
    }
}
