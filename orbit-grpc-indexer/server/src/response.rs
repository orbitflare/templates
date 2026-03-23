use axum::Json;
use axum::response::IntoResponse;
use serde::Serialize;

use indexer_core::types::Pagination;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> impl IntoResponse {
        Json(ApiResponse {
            data,
            pagination: None,
        })
    }

    pub fn paginated(data: T, pagination: Pagination) -> impl IntoResponse {
        Json(ApiResponse {
            data,
            pagination: Some(pagination),
        })
    }
}
