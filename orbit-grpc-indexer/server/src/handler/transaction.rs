use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;

use indexer_db::query::{SourceFilter, TransactionQuery};

use crate::error::ApiError;
use crate::filter::TransactionQueryParams;
use crate::pagination::{cursor, offset};
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn list_transactions(
    State(state): State<AppState>,
    Query(params): Query<TransactionQueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params
        .limit
        .unwrap_or(state.config.api.default_page_limit)
        .min(state.config.api.max_page_limit);

    let source = params.source.as_deref().and_then(SourceFilter::parse);

    let q = TransactionQuery {
        cursor: params.cursor.as_deref(),
        offset: params.offset,
        limit,
        program_id: params.program_id.as_deref(),
        account: params.account.as_deref(),
        success: params.success,
        slot_min: params.slot_min,
        slot_max: params.slot_max,
        source,
    };

    match params.pagination.as_str() {
        "offset" => {
            let (items, total) = indexer_db::query::get_transactions_offset(
                &state.db, &q,
            )
            .await?;

            let pagination = offset::build_offset_pagination(total, q.offset.unwrap_or(0), limit);
            Ok(ApiResponse::paginated(items, pagination))
        }
        _ => {
            let (items, next_cursor) = indexer_db::query::get_transactions_cursor(
                &state.db, &q,
            )
            .await?;

            let pagination = cursor::build_cursor_pagination(next_cursor, limit);
            Ok(ApiResponse::paginated(items, pagination))
        }
    }
}

pub async fn get_transaction(
    State(state): State<AppState>,
    Path(signature): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let result = indexer_db::query::get_transaction_by_signature(&state.db, &signature).await?;

    match result {
        Some((tx, inner_instructions)) => {
            let response = serde_json::json!({
                "transaction": tx,
                "inner_instructions": inner_instructions,
            });
            Ok(ApiResponse::ok(response))
        }
        None => Err(ApiError::NotFound(format!(
            "transaction {signature} not found"
        ))),
    }
}
