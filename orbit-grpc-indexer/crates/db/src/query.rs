use sea_orm::*;
use sea_orm::sea_query::Expr;

use crate::entity::{account_touched, inner_instruction, transaction};
use indexer_core::error::{IndexerError, Result};
#[derive(Debug, Clone, Copy)]
pub enum SourceFilter {
    Jetstream,
    Yellowstone,
    Both,
}

impl SourceFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Jetstream => "jetstream",
            Self::Yellowstone => "yellowstone",
            Self::Both => "both",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "jetstream" => Some(Self::Jetstream),
            "yellowstone" => Some(Self::Yellowstone),
            "both" => Some(Self::Both),
            _ => None,
        }
    }
}

pub struct TransactionQuery<'a> {
    pub cursor: Option<&'a str>,
    pub offset: Option<u64>,
    pub limit: u64,
    pub program_id: Option<&'a str>,
    pub account: Option<&'a str>,
    pub success: Option<bool>,
    pub slot_min: Option<u64>,
    pub slot_max: Option<u64>,
    pub source: Option<SourceFilter>,
}

pub async fn get_transactions_cursor(
    db: &DatabaseConnection,
    q: &TransactionQuery<'_>,
) -> Result<(Vec<transaction::Model>, Option<String>)> {
    let mut query = transaction::Entity::find()
        .order_by_desc(transaction::Column::Slot)
        .order_by_desc(transaction::Column::Signature);

    if let Some(c) = q.cursor {
        if let Some((slot_str, sig)) = c.split_once('_') {
            if let Ok(slot) = slot_str.parse::<i64>() {
                query = query.filter(
                    Condition::any()
                        .add(transaction::Column::Slot.lt(slot))
                        .add(
                            Condition::all()
                                .add(transaction::Column::Slot.eq(slot))
                                .add(transaction::Column::Signature.lt(sig.to_string())),
                        ),
                );
            }
        }
    }

    query = apply_filters(query, q);

    let rows = query
        .limit(q.limit + 1)
        .all(db)
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    let has_more = rows.len() as u64 > q.limit;
    let items: Vec<_> = rows.into_iter().take(q.limit as usize).collect();

    let next_cursor = if has_more {
        items.last().map(|t| format!("{}_{}", t.slot, t.signature))
    } else {
        None
    };

    Ok((items, next_cursor))
}

pub async fn get_transactions_offset(
    db: &DatabaseConnection,
    q: &TransactionQuery<'_>,
) -> Result<(Vec<transaction::Model>, u64)> {
    let mut query = transaction::Entity::find()
        .order_by_desc(transaction::Column::Slot);

    query = apply_filters(query, q);

    let total = query
        .clone()
        .count(db)
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    let items = query
        .offset(q.offset.unwrap_or(0))
        .limit(q.limit)
        .all(db)
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    Ok((items, total))
}

fn apply_filters(
    mut query: Select<transaction::Entity>,
    q: &TransactionQuery<'_>,
) -> Select<transaction::Entity> {
    if let Some(s) = q.success {
        query = query.filter(transaction::Column::Success.eq(s));
    }
    if let Some(min) = q.slot_min {
        query = query.filter(transaction::Column::Slot.gte(min as i64));
    }
    if let Some(max) = q.slot_max {
        query = query.filter(transaction::Column::Slot.lte(max as i64));
    }
    if let Some(acct) = q.account {
        query = query.filter(Expr::cust_with_values(
            "accounts @> $1::text[]",
            vec![format!("{{{acct}}}")],
        ));
    }
    if let Some(pid) = q.program_id {
        query = query.filter(Expr::cust_with_values(
            "accounts @> $1::text[]",
            vec![format!("{{{pid}}}")],
        ));
    }
    if let Some(source) = &q.source {
        query = query.filter(transaction::Column::Source.eq(source.as_str()));
    }
    query
}

pub async fn get_transaction_by_signature(
    db: &DatabaseConnection,
    signature: &str,
) -> Result<Option<(transaction::Model, Vec<inner_instruction::Model>)>> {
    let tx = transaction::Entity::find_by_id(signature.to_string())
        .one(db)
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    match tx {
        Some(t) => {
            let inner = inner_instruction::Entity::find()
                .filter(inner_instruction::Column::Signature.eq(signature))
                .order_by_asc(inner_instruction::Column::InstructionIdx)
                .order_by_asc(inner_instruction::Column::Depth)
                .all(db)
                .await
                .map_err(|e| IndexerError::Database(e.to_string()))?;

            Ok(Some((t, inner)))
        }
        None => Ok(None),
    }
}

pub async fn search_transactions_by_prefix(
    db: &DatabaseConnection,
    prefix: &str,
    limit: u64,
) -> Result<Vec<transaction::Model>> {
    let results = transaction::Entity::find()
        .filter(transaction::Column::Signature.starts_with(prefix))
        .order_by_desc(transaction::Column::Slot)
        .limit(limit)
        .all(db)
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    Ok(results)
}

pub async fn get_transactions_by_account_cursor(
    db: &DatabaseConnection,
    account: &str,
    cursor: Option<&str>,
    limit: u64,
) -> Result<(Vec<transaction::Model>, Option<String>)> {
    let mut at_query = account_touched::Entity::find()
        .filter(account_touched::Column::Account.eq(account))
        .order_by_desc(account_touched::Column::Slot)
        .order_by_desc(account_touched::Column::Signature);

    if let Some(c) = cursor {
        if let Some((slot_str, sig)) = c.split_once('_') {
            if let Ok(slot) = slot_str.parse::<i64>() {
                at_query = at_query.filter(
                    Condition::any()
                        .add(account_touched::Column::Slot.lt(slot))
                        .add(
                            Condition::all()
                                .add(account_touched::Column::Slot.eq(slot))
                                .add(account_touched::Column::Signature.lt(sig.to_string())),
                        ),
                );
            }
        }
    }

    let at_rows = at_query
        .limit(limit + 1)
        .all(db)
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    let has_more = at_rows.len() as u64 > limit;
    let sigs: Vec<String> = at_rows
        .iter()
        .take(limit as usize)
        .map(|r| r.signature.clone())
        .collect();

    let items = if sigs.is_empty() {
        vec![]
    } else {
        transaction::Entity::find()
            .filter(transaction::Column::Signature.is_in(sigs))
            .order_by_desc(transaction::Column::Slot)
            .all(db)
            .await
            .map_err(|e| IndexerError::Database(e.to_string()))?
    };

    let next_cursor = if has_more {
        at_rows.get(limit as usize - 1).map(|r| format!("{}_{}", r.slot, r.signature))
    } else {
        None
    };

    Ok((items, next_cursor))
}

pub async fn get_last_indexed_slot(db: &DatabaseConnection) -> Result<Option<u64>> {
    let result = transaction::Entity::find()
        .order_by_desc(transaction::Column::Slot)
        .one(db)
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    Ok(result.map(|t| t.slot as u64))
}

pub async fn get_transaction_count(db: &DatabaseConnection) -> Result<u64> {
    let count = transaction::Entity::find()
        .count(db)
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    Ok(count)
}

pub async fn is_source_active(
    db: &DatabaseConnection,
    source: &str,
    within_secs: i64,
) -> Result<bool> {
    let backend = db.get_database_backend();
    let sql = format!(
        "SELECT EXISTS(SELECT 1 FROM transactions WHERE (source = '{source}' OR source = 'both') AND indexed_at > NOW() - INTERVAL '{within_secs} seconds')"
    );
    let result = db
        .query_one(sea_orm::Statement::from_string(backend, sql))
        .await
        .map_err(|e| IndexerError::Database(e.to_string()))?;

    match result {
        Some(row) => Ok(row.try_get_by_index::<bool>(0).unwrap_or(false)),
        None => Ok(false),
    }
}
