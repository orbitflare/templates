use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub jetstream_url: String,
    pub yellowstone_url: String,
    pub database_url: String,
    pub jetstream: JetstreamConfig,
    pub yellowstone: YellowstoneConfig,
    pub database: DatabaseConfig,
    pub api: ApiConfig,
    pub filters: FilterConfig,
    pub batching: BatchConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct JetstreamConfig {
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_keepalive_secs")]
    pub tcp_keepalive_secs: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub reconnect: ReconnectConfig,
    #[serde(default)]
    pub transactions: JetstreamTransactionFilter,
    #[serde(default)]
    pub accounts: JetstreamAccountFilter,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct JetstreamTransactionFilter {
    #[serde(default)]
    pub account_include: Vec<String>,
    #[serde(default)]
    pub account_exclude: Vec<String>,
    #[serde(default)]
    pub account_required: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct JetstreamAccountFilter {
    #[serde(default)]
    pub account: Vec<String>,
    #[serde(default)]
    pub owner: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct YellowstoneConfig {
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_keepalive_secs")]
    pub tcp_keepalive_secs: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_commitment")]
    pub commitment: String,
    #[serde(default)]
    pub reconnect: ReconnectConfig,
    #[serde(default)]
    pub transactions: YellowstoneTransactionFilter,
    #[serde(default)]
    pub slots: YellowstoneSlotFilter,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct YellowstoneSlotFilter {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub filter_by_commitment: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct YellowstoneTransactionFilter {
    #[serde(default)]
    pub vote: Option<bool>,
    #[serde(default)]
    pub failed: Option<bool>,
    #[serde(default)]
    pub account_include: Vec<String>,
    #[serde(default)]
    pub account_exclude: Vec<String>,
    #[serde(default)]
    pub account_required: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReconnectConfig {
    #[serde(default = "default_reconnect_base_ms")]
    pub base_delay_ms: u64,
    #[serde(default = "default_reconnect_max_ms")]
    pub max_delay_ms: u64,
    #[serde(default = "default_reconnect_multiplier")]
    pub multiplier: f64,
    #[serde(default)]
    pub max_retries: u64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            base_delay_ms: default_reconnect_base_ms(),
            max_delay_ms: default_reconnect_max_ms(),
            multiplier: default_reconnect_multiplier(),
            max_retries: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    #[serde(default = "default_true")]
    pub run_migrations: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_default_page_limit")]
    pub default_page_limit: u64,
    #[serde(default = "default_max_page_limit")]
    pub max_page_limit: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FilterConfig {
    #[serde(default)]
    pub program_ids: Vec<String>,
    #[serde(default)]
    pub accounts: Vec<String>,
    #[serde(default)]
    pub exclude_vote_transactions: bool,
    #[serde(default)]
    pub require_success: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchConfig {
    #[serde(default = "default_batch_size")]
    pub size: usize,
    #[serde(default = "default_batch_flush_ms")]
    pub flush_interval_ms: u64,
    #[serde(default = "default_batch_retry_count")]
    pub retry_count: u32,
    #[serde(default = "default_batch_retry_delay_ms")]
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub json: bool,
}

fn default_timeout_secs() -> u64 { 30 }
fn default_keepalive_secs() -> u64 { 60 }
fn default_true() -> bool { true }
fn default_commitment() -> String { "confirmed".to_string() }
fn default_reconnect_base_ms() -> u64 { 1000 }
fn default_reconnect_max_ms() -> u64 { 30_000 }
fn default_reconnect_multiplier() -> f64 { 2.0 }
fn default_max_connections() -> u32 { 10 }
fn default_min_connections() -> u32 { 2 }
fn default_default_page_limit() -> u64 { 50 }
fn default_max_page_limit() -> u64 { 500 }
fn default_batch_size() -> usize { 100 }
fn default_batch_flush_ms() -> u64 { 500 }
fn default_batch_retry_count() -> u32 { 3 }
fn default_batch_retry_delay_ms() -> u64 { 1000 }
fn default_log_level() -> String { "info".to_string() }
