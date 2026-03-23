use serde::{Deserialize, Serialize};

use crate::types::Pagination;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorParams {
    pub cursor: Option<String>,
    pub limit: u64,
    pub direction: CursorDirection,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorDirection {
    #[default]
    Forward,
    Backward,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetParams {
    pub offset: u64,
    pub limit: u64,
}

pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}
