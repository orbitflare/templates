use indexer_core::types::{CursorMeta, Pagination};

pub fn build_cursor_pagination(next_cursor: Option<String>, limit: u64) -> Pagination {
    let has_more = next_cursor.is_some();
    Pagination::Cursor(CursorMeta {
        next_cursor,
        limit,
        has_more,
    })
}
