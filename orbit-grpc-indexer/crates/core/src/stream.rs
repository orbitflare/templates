use async_trait::async_trait;

use crate::error::Result;
use crate::types::{RawTransaction, StreamSource};

#[async_trait]
pub trait TransactionStream: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn next(&mut self) -> Result<Option<RawTransaction>>;
    async fn reconnect(&mut self) -> Result<()>;
    fn source(&self) -> StreamSource;
}
