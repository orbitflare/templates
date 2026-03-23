use std::path::Path;
use std::time::Duration;

use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use tracing::info;

use indexer_config::model::DatabaseConfig;
use indexer_core::error::{IndexerError, Result};

pub async fn create_pool(url: &str, config: &DatabaseConfig) -> Result<DatabaseConnection> {
    let mut opts = ConnectOptions::new(url);
    opts.max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .connect_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(600))
        .sqlx_logging(false);

    let db = Database::connect(opts)
        .await
        .map_err(|e| IndexerError::Database(format!("connection failed: {e}")))?;

    info!("database pool created");
    Ok(db)
}

pub async fn run_migrations(db: &DatabaseConnection, migrations_dir: &Path) -> Result<()> {
    let backend = db.get_database_backend();

    let mut entries: Vec<_> = std::fs::read_dir(migrations_dir)
        .map_err(|e| IndexerError::Database(format!("failed to read migrations dir: {e}")))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "sql")
        })
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        let sql = std::fs::read_to_string(&path)
            .map_err(|e| IndexerError::Database(format!("failed to read {name}: {e}")))?;

        info!(migration = %name, "running migration");

        for statement in sql.split(';') {
            let trimmed = statement.trim();
            if trimmed.is_empty() {
                continue;
            }
            let _: sea_orm::ExecResult = db
                .execute(sea_orm::Statement::from_string(backend, trimmed.to_string()))
                .await
                .map_err(|e| IndexerError::Database(format!("migration {name} failed: {e}")))?;
        }
    }

    info!("migrations done");
    Ok(())
}
