use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::broadcast;

use indexer_core::health::HealthReporter;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Arc<indexer_config::model::AppConfig>,
    pub health: Arc<dyn HealthReporter>,
    pub tx_broadcast: broadcast::Sender<String>,
}
