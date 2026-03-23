use indexer_core::types::{OffsetMeta, Pagination};

pub fn build_offset_pagination(total: u64, offset: u64, limit: u64) -> Pagination {
    Pagination::Offset(OffsetMeta {
        total,
        offset,
        limit,
        has_more: offset + limit < total,
    })
}
