use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "copy-trader", about = "Solana copy trading engine")]
pub struct Cli {
    /// Config file path
    #[arg(short, long, default_value = "config.yml")]
    pub config: PathBuf,

    /// Force dry run mode (overrides config)
    #[arg(short, long)]
    pub dry_run: bool,

    /// Increase log verbosity (-v = debug, -vv = trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Track a single wallet (overrides config targets)
    #[arg(long)]
    pub wallet: Option<String>,

    /// Validate config and exit
    #[arg(long)]
    pub validate: bool,

    /// Run database migrations and exit
    #[arg(long)]
    pub migrate: bool,
}

// ── Config enums ──

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SizingMode {
    #[default]
    Fixed,
    Proportional,
    Mirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeeStrategy {
    Fixed,
    #[default]
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmStrategy {
    #[default]
    Poll,
    Websocket,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    Pretty,
    #[default]
    Json,
}

// ── Config structs ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub jetstream: JetstreamConfig,
    pub rpc: RpcConfig,
    #[serde(default)]
    pub targets: Vec<TargetConfig>,
    pub execution: ExecutionConfig,
    pub safety: SafetyConfig,
    pub decoders: DecodersConfig,
    pub redis: RedisConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub journal: JournalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JetstreamConfig {
    pub url: String,
    #[serde(default)]
    pub timeout_secs: u64,
    #[serde(default)]
    pub tcp_keepalive_secs: u64,
    #[serde(default)]
    pub channel_buffer_size: usize,
    #[serde(default)]
    pub reconnect: ReconnectConfig,
}

impl Default for JetstreamConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            timeout_secs: 30,
            tcp_keepalive_secs: 60,
            channel_buffer_size: 10000,
            reconnect: ReconnectConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectConfig {
    #[serde(default)]
    pub initial_delay_ms: u64,
    #[serde(default)]
    pub max_delay_ms: u64,
    #[serde(default)]
    pub multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay_ms: 100,
            max_delay_ms: 30000,
            multiplier: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub url: String,
    #[serde(default)]
    pub timeout_secs: u64,
    #[serde(default)]
    pub max_retries: u32,
    #[serde(default)]
    pub commitment: String,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            timeout_secs: 10,
            max_retries: 3,
            commitment: "confirmed".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    pub address: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub sizing: Option<SizingOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizingOverride {
    #[serde(default)]
    pub mode: SizingMode,
    #[serde(default)]
    pub fixed_amount_sol: Option<f64>,
    #[serde(default)]
    pub proportion: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    pub sizing: SizingConfig,
    #[serde(default)]
    pub slippage: SlippageConfig,
    #[serde(default)]
    pub priority_fee: PriorityFeeConfig,
    #[serde(default)]
    pub jito: JitoConfig,
    #[serde(default)]
    pub confirmation: ConfirmationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizingConfig {
    #[serde(default)]
    pub mode: SizingMode,
    #[serde(default)]
    pub fixed_amount_sol: f64,
    #[serde(default)]
    pub proportion: f64,
    #[serde(default)]
    pub max_trade_sol: f64,
    #[serde(default)]
    pub min_trade_sol: f64,
}

impl Default for SizingConfig {
    fn default() -> Self {
        Self {
            mode: SizingMode::Fixed,
            fixed_amount_sol: 0.1,
            proportion: 0.1,
            max_trade_sol: 5.0,
            min_trade_sol: 0.01,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageConfig {
    #[serde(default)]
    pub default_bps: u32,
    #[serde(default)]
    pub max_bps: u32,
}

impl Default for SlippageConfig {
    fn default() -> Self {
        Self {
            default_bps: 300,
            max_bps: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityFeeConfig {
    #[serde(default)]
    pub strategy: FeeStrategy,
    #[serde(default)]
    pub fixed_lamports: u64,
    #[serde(default)]
    pub dynamic_percentile: u32,
    #[serde(default)]
    pub max_lamports: u64,
}

impl Default for PriorityFeeConfig {
    fn default() -> Self {
        Self {
            strategy: FeeStrategy::Dynamic,
            fixed_lamports: 100000,
            dynamic_percentile: 75,
            max_lamports: 5000000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub tip_lamports: u64,
    #[serde(default)]
    pub block_engine_url: String,
}

impl Default for JitoConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tip_lamports: 50000,
            block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationConfig {
    #[serde(default)]
    pub strategy: ConfirmStrategy,
    #[serde(default)]
    pub timeout_secs: u64,
    #[serde(default)]
    pub poll_interval_ms: u64,
}

impl Default for ConfirmationConfig {
    fn default() -> Self {
        Self {
            strategy: ConfirmStrategy::Poll,
            timeout_secs: 30,
            poll_interval_ms: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    #[serde(default)]
    pub max_daily_trades: u32,
    #[serde(default)]
    pub max_hourly_trades: u32,
    #[serde(default)]
    pub cooldown_per_token_secs: u64,
    #[serde(default)]
    pub max_open_positions: u32,
    #[serde(default)]
    pub max_portfolio_sol: f64,
    #[serde(default)]
    pub token_filters: TokenFilterConfig,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_daily_trades: 100,
            max_hourly_trades: 20,
            cooldown_per_token_secs: 60,
            max_open_positions: 10,
            max_portfolio_sol: 50.0,
            token_filters: TokenFilterConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenFilterConfig {
    #[serde(default)]
    pub blacklisted_tokens: Vec<String>,
    #[serde(default)]
    pub min_liquidity_sol: f64,
    #[serde(default)]
    pub require_mint_renounced: bool,
    #[serde(default = "default_true")]
    pub require_freeze_renounced: bool,
    #[serde(default)]
    pub min_token_age_secs: u64,
}

impl Default for TokenFilterConfig {
    fn default() -> Self {
        Self {
            blacklisted_tokens: Vec::new(),
            min_liquidity_sol: 10.0,
            require_mint_renounced: false,
            require_freeze_renounced: true,
            min_token_age_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodersConfig {
    #[serde(default)]
    pub jupiter: DecoderEntry,
    #[serde(default)]
    pub raydium_amm: DecoderEntry,
    #[serde(default)]
    pub raydium_cpmm: DecoderEntry,
    #[serde(default)]
    pub pumpfun: DecoderEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoderEntry {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub program_id: String,
}

impl Default for DecoderEntry {
    fn default() -> Self {
        Self {
            enabled: true,
            program_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub dedup_ttl_secs: u64,
    #[serde(default)]
    pub price_cache_ttl_secs: u64,
    #[serde(default)]
    pub position_sync_interval_secs: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            prefix: "copytrader:".to_string(),
            dedup_ttl_secs: 300,
            price_cache_ttl_secs: 10,
            position_sync_interval_secs: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub listen: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            listen: "0.0.0.0:9090".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub format: LogFormat,
    #[serde(default = "default_true")]
    pub log_transactions: bool,
    #[serde(default = "default_true")]
    pub log_simulations: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Json,
            log_transactions: true,
            log_simulations: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub telegram: TelegramConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub chat_id: String,
    #[serde(default)]
    pub notify_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JournalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub database_url: String,
}

fn default_true() -> bool { true }

pub fn load_config(path: &std::path::Path) -> anyhow::Result<AppConfig> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file {:?}: {}", path, e))?;

    let expanded = shellexpand::env(&raw)
        .map_err(|e| anyhow::anyhow!("Failed to expand env vars in config: {}", e))?;

    let config: AppConfig = serde_yml::from_str(&expanded)
        .map_err(|e| anyhow::anyhow!("Failed to parse config YAML: {}", e))?;

    Ok(config)
}

pub fn apply_cli_overrides(mut config: AppConfig, cli: &Cli) -> AppConfig {
    if let Ok(val) = std::env::var("DRY_RUN") {
        match val.to_lowercase().as_str() {
            "false" | "0" | "no" => config.execution.dry_run = false,
            "true" | "1" | "yes" => config.execution.dry_run = true,
            _ => {}
        }
    }

    if cli.dry_run {
        config.execution.dry_run = true;
    }

    if let Some(ref wallet) = cli.wallet {
        config.targets = vec![TargetConfig {
            address: wallet.clone(),
            label: Some("cli-target".to_string()),
            enabled: true,
            sizing: None,
        }];
    }

    config
}

pub fn validate_config(config: &AppConfig) -> anyhow::Result<()> {
    if config.jetstream.url.is_empty() {
        anyhow::bail!("jetstream.url is required");
    }
    if config.rpc.url.is_empty() {
        anyhow::bail!("rpc.url is required");
    }
    if config.targets.is_empty() {
        anyhow::bail!("At least one target wallet is required");
    }
    for (i, target) in config.targets.iter().enumerate() {
        if target.address.is_empty() {
            anyhow::bail!("targets[{}].address is required", i);
        }
    }
    if config.redis.url.is_empty() {
        anyhow::bail!("redis.url is required");
    }
    if config.execution.sizing.max_trade_sol <= 0.0 {
        anyhow::bail!("execution.sizing.max_trade_sol must be positive");
    }
    if config.execution.sizing.min_trade_sol <= 0.0 {
        anyhow::bail!("execution.sizing.min_trade_sol must be positive");
    }

    tracing::info!("Configuration validated successfully");
    tracing::info!("  Targets: {}", config.targets.len());
    tracing::info!("  Dry run: {}", config.execution.dry_run);
    tracing::info!("  Sizing mode: {:?}", config.execution.sizing.mode);
    tracing::info!("  Journal enabled: {}", config.journal.enabled);
    tracing::info!("  Telegram enabled: {}", config.notifications.telegram.enabled);

    Ok(())
}
