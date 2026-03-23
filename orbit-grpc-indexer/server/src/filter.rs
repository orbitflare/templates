use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TransactionQueryParams {
    pub program_id: Option<String>,
    pub account: Option<String>,
    pub success: Option<bool>,
    pub slot_min: Option<u64>,
    pub slot_max: Option<u64>,
    pub source: Option<String>,
    pub cursor: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
    #[serde(default = "default_pagination_style")]
    pub pagination: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountQueryParams {
    pub cursor: Option<String>,
    pub limit: Option<u64>,
}

fn default_pagination_style() -> String {
    "cursor".to_string()
}
