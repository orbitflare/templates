use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use tracing::{error, warn};

use indexer_config::model::BatchConfig;
use indexer_core::error::{IndexerError, Result};
use indexer_core::sink::TransactionSink;
use indexer_core::types::ProcessedTransaction;

pub struct PostgresSink {
    db: DatabaseConnection,
    batch_config: BatchConfig,
}

impl PostgresSink {
    pub fn new(db: DatabaseConnection, batch_config: BatchConfig) -> Self {
        Self { db, batch_config }
    }

    async fn upsert_transaction(&self, tx: &ProcessedTransaction) -> Result<()> {
        let backend = self.db.get_database_backend();

        let sql = r#"
            INSERT INTO transactions (
                signature, slot, block_time, fee, success, err,
                num_instructions, accounts, log_messages,
                has_cpi_data, source, raw, indexed_at
            ) VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8::text[], $9::text[], $10, $11, $12::jsonb, $13)
            ON CONFLICT (signature) DO UPDATE SET
                fee = COALESCE(EXCLUDED.fee, transactions.fee),
                success = EXCLUDED.success,
                err = COALESCE(EXCLUDED.err, transactions.err),
                log_messages = CASE
                    WHEN array_length(EXCLUDED.log_messages, 1) IS NOT NULL
                    THEN EXCLUDED.log_messages
                    ELSE transactions.log_messages
                END,
                has_cpi_data = transactions.has_cpi_data OR EXCLUDED.has_cpi_data,
                source = CASE
                    WHEN transactions.source != EXCLUDED.source THEN 'both'
                    ELSE EXCLUDED.source
                END,
                raw = COALESCE(EXCLUDED.raw, transactions.raw),
                enriched_at = CASE
                    WHEN EXCLUDED.has_cpi_data AND NOT transactions.has_cpi_data
                    THEN NOW()
                    ELSE transactions.enriched_at
                END
        "#;

        let now = Utc::now();
        let accounts_array = format!(
            "{{{}}}",
            tx.account_keys
                .iter()
                .map(|a| format!("\"{}\"", a))
                .collect::<Vec<_>>()
                .join(",")
        );
        let logs_array = format!(
            "{{{}}}",
            tx.log_messages
                .iter()
                .map(|l| format!("\"{}\"", l.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(",")
        );

        let err_json = tx.err.as_ref().map(|e| e.to_string());
        let raw_json = tx.raw.as_ref().map(|r| r.to_string());

        self.db
            .execute(Statement::from_sql_and_values(
                backend,
                sql,
                vec![
                    tx.signature.clone().into(),
                    (tx.slot as i64).into(),
                    tx.block_time.into(),
                    tx.fee.map(|f| f as i64).into(),
                    tx.success.into(),
                    err_json.into(),
                    (tx.num_instructions as i32).into(),
                    accounts_array.into(),
                    logs_array.into(),
                    tx.has_cpi_data.into(),
                    tx.source.to_string().into(),
                    raw_json.into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| IndexerError::Database(format!("upsert failed: {e}")))?;

        Ok(())
    }

    async fn insert_inner_instructions(&self, tx: &ProcessedTransaction) -> Result<()> {
        if tx.inner_instructions.is_empty() {
            return Ok(());
        }

        let backend = self.db.get_database_backend();

        for ii in &tx.inner_instructions {
            let accounts_array = format!(
                "{{{}}}",
                ii.accounts
                    .iter()
                    .map(|a| format!("\"{}\"", a))
                    .collect::<Vec<_>>()
                    .join(",")
            );

            let sql = r#"
                INSERT INTO inner_instructions (
                    signature, instruction_idx, depth, program_id, accounts, data
                ) VALUES ($1, $2, $3, $4, $5::text[], $6)
                ON CONFLICT DO NOTHING
            "#;

            self.db
                .execute(Statement::from_sql_and_values(
                    backend,
                    sql,
                    vec![
                        tx.signature.clone().into(),
                        (ii.instruction_index as i32).into(),
                        (ii.depth as i32).into(),
                        ii.program_id.clone().into(),
                        accounts_array.into(),
                        ii.data.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| IndexerError::Database(format!("inner instruction insert failed: {e}")))?;
        }

        Ok(())
    }

    async fn insert_accounts_touched(&self, tx: &ProcessedTransaction) -> Result<()> {
        if tx.account_keys.is_empty() {
            return Ok(());
        }

        let backend = self.db.get_database_backend();

        for (i, account) in tx.account_keys.iter().enumerate() {
            let sql = r#"
                INSERT INTO accounts_touched (
                    account, signature, slot, is_signer, is_writable
                ) VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (account, signature) DO NOTHING
            "#;

            self.db
                .execute(Statement::from_sql_and_values(
                    backend,
                    sql,
                    vec![
                        account.clone().into(),
                        tx.signature.clone().into(),
                        (tx.slot as i64).into(),
                        (i == 0).into(),
                        (i == 0).into(),
                    ],
                ))
                .await
                .map_err(|e| IndexerError::Database(format!("accounts_touched insert failed: {e}")))?;
        }

        Ok(())
    }

    async fn write_with_retry(&self, tx: &ProcessedTransaction) -> Result<()> {
        let mut last_err = None;

        for attempt in 0..=self.batch_config.retry_count {
            match self.write_single(tx).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(
                        attempt = attempt + 1,
                        max = self.batch_config.retry_count,
                        error = %e,
                        signature = %tx.signature,
                        "write failed, retrying"
                    );
                    last_err = Some(e);

                    if attempt < self.batch_config.retry_count {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            self.batch_config.retry_delay_ms,
                        ))
                        .await;
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| IndexerError::Database("write failed".into())))
    }

    async fn write_single(&self, tx: &ProcessedTransaction) -> Result<()> {
        self.upsert_transaction(tx).await?;
        self.insert_inner_instructions(tx).await?;
        self.insert_accounts_touched(tx).await?;
        self.notify(tx).await;
        Ok(())
    }

    async fn notify(&self, tx: &ProcessedTransaction) {
        let payload = serde_json::json!({
            "signature": &tx.signature,
            "slot": tx.slot,
            "source": tx.source.to_string(),
            "success": tx.success,
            "has_cpi_data": tx.has_cpi_data,
        });
        let sql = format!(
            "NOTIFY new_transaction, '{}'",
            payload.to_string().replace('\'', "''")
        );
        let backend = self.db.get_database_backend();
        let _ = self.db.execute(Statement::from_string(backend, sql)).await;
    }
}

#[async_trait]
impl TransactionSink for PostgresSink {
    async fn write(&self, tx: &ProcessedTransaction) -> Result<()> {
        self.write_with_retry(tx).await
    }

    async fn write_batch(&self, txs: &[ProcessedTransaction]) -> Result<()> {
        for tx in txs {
            if let Err(e) = self.write_with_retry(tx).await {
                error!(
                    signature = %tx.signature,
                    error = %e,
                    "batch write failed for transaction"
                );
            }
        }
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "postgres"
    }
}
