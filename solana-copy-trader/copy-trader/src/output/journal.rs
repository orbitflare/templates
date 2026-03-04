use crate::config::AppConfig;
use crate::types::TradeRecord;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub struct TradeJournal {
    pool: PgPool,
    record_rx: mpsc::Receiver<TradeRecord>,
    shutdown_rx: watch::Receiver<bool>,
}

impl TradeJournal {
    pub async fn new(
        config: Arc<AppConfig>,
        record_rx: mpsc::Receiver<TradeRecord>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.journal.database_url)
            .await?;

        tracing::info!("Connected to PostgreSQL journal database");

        Ok(Self {
            pool,
            record_rx,
            shutdown_rx,
        })
    }

    pub async fn run_migrations(database_url: &str) -> anyhow::Result<()> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await?;

        let migration_sql = include_str!("../../../migrations/001_init.sql");
        sqlx::raw_sql(migration_sql).execute(&pool).await?;

        tracing::info!("Database migrations applied successfully");
        Ok(())
    }

    pub async fn run(mut self) {
        tracing::info!("TradeJournal writer started");

        loop {
            tokio::select! {
                Some(record) = self.record_rx.recv() => {
                    if let Err(e) = self.write_record(&record).await {
                        tracing::error!(error = %e, "Failed to write trade record to journal");
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    tracing::info!("TradeJournal shutting down, flushing pending writes...");
                    while let Ok(record) = self.record_rx.try_recv() {
                        if let Err(e) = self.write_record(&record).await {
                            tracing::error!(error = %e, "Failed to flush trade record");
                        }
                    }
                    tracing::info!("TradeJournal shutdown complete");
                    return;
                }
            }
        }
    }

    async fn write_record(&self, record: &TradeRecord) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO trades (
                target_wallet, target_label, target_tx_sig,
                direction, dex, input_mint, output_mint,
                target_amount, our_amount_sol, our_tx_sig,
                status, failure_reason, slippage_bps,
                priority_fee, latency_ms, dry_run
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (target_tx_sig) DO NOTHING
            "#,
        )
        .bind(&record.target_wallet)
        .bind(&record.target_label)
        .bind(&record.target_tx_sig)
        .bind(record.direction.to_string())
        .bind(record.dex.to_string())
        .bind(&record.input_mint)
        .bind(&record.output_mint)
        .bind(record.target_amount)
        .bind(record.our_amount_sol)
        .bind(&record.our_tx_sig)
        .bind(record.status.to_string())
        .bind(&record.failure_reason)
        .bind(record.slippage_bps)
        .bind(record.priority_fee)
        .bind(record.latency_ms)
        .bind(record.dry_run)
        .execute(&self.pool)
        .await?;

        tracing::debug!(
            target_tx = %record.target_tx_sig,
            status = %record.status,
            "Trade record written to journal"
        );

        Ok(())
    }
}
