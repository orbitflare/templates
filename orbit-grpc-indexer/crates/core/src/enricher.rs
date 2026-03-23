use async_trait::async_trait;

use crate::error::Result;
use crate::types::{ProcessedTransaction, RawTransaction};

#[async_trait]
pub trait TransactionEnricher: Send + Sync {
    async fn enrich(&self, tx: RawTransaction) -> Result<ProcessedTransaction>;
    fn name(&self) -> &str;
}
