use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::error::ApiError;
use crate::state::AppState;

pub async fn health_check(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let status = state.health.report().await?;
    Ok(Json(status))
}
