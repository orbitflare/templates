use async_trait::async_trait;

use crate::error::Result;
use crate::types::ProcessedTransaction;

#[async_trait]
pub trait TransactionSink: Send + Sync {
    async fn write(&self, tx: &ProcessedTransaction) -> Result<()>;
    async fn write_batch(&self, txs: &[ProcessedTransaction]) -> Result<()>;
    async fn flush(&self) -> Result<()>;
    fn name(&self) -> &str;
}
