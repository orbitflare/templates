use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use indexer_core::error::Result;
use indexer_core::health::HealthReporter;
use indexer_core::types::HealthStatus;

pub struct SystemHealth {
    db: DatabaseConnection,
}

impl SystemHealth {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl HealthReporter for SystemHealth {
    async fn report(&self) -> Result<HealthStatus> {
        let (last_slot, tx_count, js, ys) = tokio::join!(
            indexer_db::query::get_last_indexed_slot(&self.db),
            indexer_db::query::get_transaction_count(&self.db),
            indexer_db::query::is_source_active(&self.db, "jetstream", 5),
            indexer_db::query::is_source_active(&self.db, "yellowstone", 5),
        );

        let last_slot = last_slot.unwrap_or(None);
        let tx_count = tx_count.unwrap_or(0);
        let js = js.unwrap_or(false);
        let ys = ys.unwrap_or(false);
        let healthy = (js || ys) && last_slot.is_some();

        Ok(HealthStatus {
            healthy,
            jetstream_connected: js,
            yellowstone_connected: ys,
            database_connected: true,
            last_indexed_slot: last_slot,
            transactions_indexed: tx_count,
        })
    }
}
