use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;

use crate::error::ApiError;
use crate::filter::AccountQueryParams;
use crate::pagination::cursor;
use crate::response::ApiResponse;
use crate::state::AppState;

pub async fn get_account_transactions(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(params): Query<AccountQueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params
        .limit
        .unwrap_or(state.config.api.default_page_limit)
        .min(state.config.api.max_page_limit);

    let (items, next_cursor) = indexer_db::query::get_transactions_by_account_cursor(
        &state.db,
        &address,
        params.cursor.as_deref(),
        limit,
    )
    .await?;

    let pagination = cursor::build_cursor_pagination(next_cursor, limit);
    Ok(ApiResponse::paginated(items, pagination))
}
