use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamSource {
    Jetstream,
    Yellowstone,
}

impl std::fmt::Display for StreamSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jetstream => write!(f, "jetstream"),
            Self::Yellowstone => write!(f, "yellowstone"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<DateTime<Utc>>,
    pub fee: Option<u64>,
    pub success: bool,
    pub err: Option<serde_json::Value>,
    pub num_instructions: u32,
    pub account_keys: Vec<String>,
    pub log_messages: Vec<String>,
    pub inner_instructions: Vec<InnerInstruction>,
    pub raw: Option<serde_json::Value>,
    pub source: StreamSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnerInstruction {
    pub instruction_index: u32,
    pub depth: u32,
    pub program_id: String,
    pub accounts: Vec<String>,
    pub data: String,
}

#[derive(Debug, Clone)]
pub struct ProcessedTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<DateTime<Utc>>,
    pub fee: Option<u64>,
    pub success: bool,
    pub err: Option<serde_json::Value>,
    pub num_instructions: u32,
    pub account_keys: Vec<String>,
    pub log_messages: Vec<String>,
    pub inner_instructions: Vec<InnerInstruction>,
    pub has_cpi_data: bool,
    pub source: StreamSource,
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub jetstream_connected: bool,
    pub yellowstone_connected: bool,
    pub database_connected: bool,
    pub last_indexed_slot: Option<u64>,
    pub transactions_indexed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Pagination {
    Cursor(CursorMeta),
    Offset(OffsetMeta),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorMeta {
    pub next_cursor: Option<String>,
    pub limit: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetMeta {
    pub total: u64,
    pub offset: u64,
    pub limit: u64,
    pub has_more: bool,
}
