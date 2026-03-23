use std::path::Path;

use crate::model::AppConfig;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Read(#[from] std::io::Error),

    #[error("missing environment variable: {0}")]
    MissingEnv(String),

    #[error("failed to parse config: {0}")]
    Parse(String),

    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(serde::Deserialize)]
struct YamlConfig {
    #[serde(default)]
    jetstream: crate::model::JetstreamConfig,
    #[serde(default)]
    yellowstone: crate::model::YellowstoneConfig,
    #[serde(default)]
    database: crate::model::DatabaseConfig,
    #[serde(default)]
    api: crate::model::ApiConfig,
    #[serde(default)]
    filters: crate::model::FilterConfig,
    #[serde(default)]
    batching: crate::model::BatchConfig,
    #[serde(default)]
    logging: crate::model::LoggingConfig,
}

impl Default for crate::model::ApiConfig {
    fn default() -> Self {
        Self {
            default_page_limit: 50,
            max_page_limit: 500,
        }
    }
}

impl Default for crate::model::BatchConfig {
    fn default() -> Self {
        Self {
            size: 100,
            flush_interval_ms: 500,
            retry_count: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl Default for crate::model::LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json: false,
        }
    }
}

pub fn load_config(path: &Path) -> Result<AppConfig, ConfigError> {
    let _ = dotenvy::dotenv();

    let jetstream_url = require_env("JETSTREAM_URL")?;
    let yellowstone_url = require_env("YELLOWSTONE_URL")?;
    let database_url = require_env("DATABASE_URL")?;

    let raw = std::fs::read_to_string(path)?;
    let yaml: YamlConfig =
        serde_yml::from_str(&raw).map_err(|e| ConfigError::Parse(e.to_string()))?;

    let config = AppConfig {
        jetstream_url,
        yellowstone_url,
        database_url,
        jetstream: yaml.jetstream,
        yellowstone: yaml.yellowstone,
        database: yaml.database,
        api: yaml.api,
        filters: yaml.filters,
        batching: yaml.batching,
        logging: yaml.logging,
    };

    validate(&config)?;
    Ok(config)
}

fn require_env(key: &str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError::MissingEnv(key.to_string()))
}

fn validate(config: &AppConfig) -> Result<(), ConfigError> {
    let valid_commitments = ["processed", "confirmed", "finalized"];
    if !valid_commitments.contains(&config.yellowstone.commitment.as_str()) {
        return Err(ConfigError::Validation(format!(
            "yellowstone.commitment must be one of: {}",
            valid_commitments.join(", ")
        )));
    }
    Ok(())
}
