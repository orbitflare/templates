use async_trait::async_trait;

use crate::error::Result;
use crate::types::HealthStatus;

#[async_trait]
pub trait HealthReporter: Send + Sync {
    async fn report(&self) -> Result<HealthStatus>;
}
