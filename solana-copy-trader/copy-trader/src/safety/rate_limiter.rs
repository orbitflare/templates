use crate::config::AppConfig;
use crate::safety::SafetyError;
use crate::state::redis::RedisClient;
use crate::types::TradeIntent;
use std::sync::Arc;

pub struct RateLimiter {
    config: Arc<AppConfig>,
    redis: Arc<RedisClient>,
}

impl RateLimiter {
    pub fn new(config: Arc<AppConfig>, redis: Arc<RedisClient>) -> Self {
        Self { config, redis }
    }

    pub async fn check(&self, intent: &TradeIntent) -> Result<(), SafetyError> {
        let mint = intent.output_mint.to_string();

        if self.redis.is_on_cooldown(&mint).await.unwrap_or(false) {
            return Err(SafetyError::Cooldown(mint));
        }

        let hourly = self.redis.get_hourly_trade_count().await.unwrap_or(0);
        if hourly >= self.config.safety.max_hourly_trades {
            return Err(SafetyError::HourlyLimit);
        }

        let daily = self.redis.get_daily_trade_count().await.unwrap_or(0);
        if daily >= self.config.safety.max_daily_trades {
            return Err(SafetyError::DailyLimit);
        }

        let positions = self.redis.get_open_position_count().await.unwrap_or(0);
        if positions >= self.config.safety.max_open_positions {
            return Err(SafetyError::MaxPositions);
        }

        let max_portfolio = self.config.safety.max_portfolio_sol;
        if max_portfolio > 0.0 {
            let all_positions = self.redis.get_all_positions().await.unwrap_or_default();
            let total_exposure: f64 = all_positions.iter().map(|p| p.entry_amount_sol).sum();
            if total_exposure >= max_portfolio {
                return Err(SafetyError::MaxPortfolio {
                    current_sol: total_exposure,
                    max_sol: max_portfolio,
                });
            }
        }

        Ok(())
    }
}
