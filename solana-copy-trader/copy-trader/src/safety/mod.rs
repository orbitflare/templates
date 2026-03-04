pub mod token_filter;
pub mod rate_limiter;

use crate::config::AppConfig;
use crate::state::redis::RedisClient;
use crate::types::TradeIntent;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum SafetyError {
    #[error("token blacklisted: {0}")]
    Blacklisted(String),
    #[error("token cooldown active: {0}")]
    Cooldown(String),
    #[error("hourly trade limit reached")]
    HourlyLimit,
    #[error("daily trade limit reached")]
    DailyLimit,
    #[error("max open positions reached")]
    MaxPositions,
    #[error("max portfolio exposure reached ({current_sol:.2} SOL >= {max_sol:.2} SOL)")]
    MaxPortfolio { current_sol: f64, max_sol: f64 },
    #[error("mint authority not renounced for {0}")]
    MintAuthority(String),
    #[error("freeze authority not renounced for {0}")]
    FreezeAuthority(String),
    #[error("token too new: {mint} is {age_secs}s old, min {min_secs}s required")]
    TokenTooNew {
        mint: String,
        age_secs: u64,
        min_secs: u64,
    },
    #[error("min liquidity not met for {0}")]
    MinLiquidity(String),
}

pub struct SafetyFilter {
    token_filter: token_filter::TokenFilter,
    rate_limiter: rate_limiter::RateLimiter,
}

impl SafetyFilter {
    pub fn new(config: Arc<AppConfig>, redis: Arc<RedisClient>) -> Self {
        Self {
            token_filter: token_filter::TokenFilter::new(config.clone()),
            rate_limiter: rate_limiter::RateLimiter::new(config, redis),
        }
    }

    pub async fn check(&self, intent: &TradeIntent) -> Result<(), SafetyError> {
        self.token_filter.check(intent).await?;
        self.rate_limiter.check(intent).await?;

        Ok(())
    }
}
