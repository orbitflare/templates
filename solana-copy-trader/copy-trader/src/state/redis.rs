use crate::config::AppConfig;
use crate::types::Position;
use chrono::Utc;
use redis::AsyncCommands;
use std::sync::Arc;

pub struct RedisClient {
    pool: redis::aio::ConnectionManager,
    config: Arc<AppConfig>,
}

impl RedisClient {
    pub async fn connect(config: Arc<AppConfig>) -> anyhow::Result<Self> {
        let client = redis::Client::open(config.redis.url.as_str())?;
        let pool = redis::aio::ConnectionManager::new(client).await?;
        tracing::info!(url = %config.redis.url, "Connected to Redis");
        Ok(Self { pool, config })
    }

    fn key(&self, suffix: &str) -> String {
        format!("{}{}", self.config.redis.prefix, suffix)
    }

    /// Returns true if this tx signature has already been seen (duplicate).
    pub async fn check_dedup(&self, tx_sig: &str) -> anyhow::Result<bool> {
        let key = self.key(&format!("dedup:{}", tx_sig));
        let mut conn = self.pool.clone();
        let existed: bool = conn
            .set_nx(&key, 1i32)
            .await?;
        if existed {
            conn.expire::<_, ()>(&key, self.config.redis.dedup_ttl_secs as i64).await?;
            Ok(false)
        } else {
            Ok(true)
        }
    }

    pub async fn get_cached_quote(&self, mint: &str) -> anyhow::Result<Option<String>> {
        let key = self.key(&format!("price:{}", mint));
        let mut conn = self.pool.clone();
        let val: Option<String> = conn.get(&key).await?;
        Ok(val)
    }

    pub async fn set_cached_quote(&self, mint: &str, quote_json: &str) -> anyhow::Result<()> {
        let key = self.key(&format!("price:{}", mint));
        let mut conn = self.pool.clone();
        conn.set_ex::<_, _, ()>(&key, quote_json, self.config.redis.price_cache_ttl_secs).await?;
        Ok(())
    }

    pub async fn get_open_position_count(&self) -> anyhow::Result<u32> {
        let key = self.key("positions");
        let mut conn = self.pool.clone();
        let count: u32 = conn.hlen(&key).await.unwrap_or(0);
        Ok(count)
    }

    pub async fn add_position(&self, position: &Position) -> anyhow::Result<()> {
        let key = self.key("positions");
        let mut conn = self.pool.clone();
        let data = serde_json::to_string(position)?;
        conn.hset::<_, _, _, ()>(&key, &position.mint, data).await?;
        Ok(())
    }

    pub async fn remove_position(&self, mint: &str) -> anyhow::Result<()> {
        let key = self.key("positions");
        let mut conn = self.pool.clone();
        conn.hdel::<_, _, ()>(&key, mint).await?;
        Ok(())
    }

    pub async fn get_all_positions(&self) -> anyhow::Result<Vec<Position>> {
        let key = self.key("positions");
        let mut conn = self.pool.clone();
        let map: std::collections::HashMap<String, String> = conn.hgetall(&key).await.unwrap_or_default();
        let mut positions = Vec::new();
        for (_mint, data) in map {
            if let Ok(pos) = serde_json::from_str::<Position>(&data) {
                positions.push(pos);
            }
        }
        Ok(positions)
    }

    pub async fn increment_trade_count(&self) -> anyhow::Result<()> {
        let now = Utc::now();
        let date = now.format("%Y-%m-%d").to_string();
        let hour = now.format("%H").to_string();

        let hourly_key = self.key(&format!("rate:{}:{}", date, hour));
        let daily_key = self.key(&format!("rate:{}", date));

        let mut conn = self.pool.clone();
        conn.incr::<_, _, ()>(&hourly_key, 1i32).await?;
        conn.expire::<_, ()>(&hourly_key, 7200).await?;
        conn.incr::<_, _, ()>(&daily_key, 1i32).await?;
        conn.expire::<_, ()>(&daily_key, 86400).await?;

        Ok(())
    }

    pub async fn get_hourly_trade_count(&self) -> anyhow::Result<u32> {
        let now = Utc::now();
        let key = self.key(&format!("rate:{}:{}", now.format("%Y-%m-%d"), now.format("%H")));
        let mut conn = self.pool.clone();
        let count: u32 = conn.get(&key).await.unwrap_or(0);
        Ok(count)
    }

    pub async fn get_daily_trade_count(&self) -> anyhow::Result<u32> {
        let now = Utc::now();
        let key = self.key(&format!("rate:{}", now.format("%Y-%m-%d")));
        let mut conn = self.pool.clone();
        let count: u32 = conn.get(&key).await.unwrap_or(0);
        Ok(count)
    }

    pub async fn set_cooldown(&self, mint: &str) -> anyhow::Result<()> {
        let key = self.key(&format!("cooldown:{}", mint));
        let mut conn = self.pool.clone();
        conn.set_ex::<_, _, ()>(&key, 1i32, self.config.safety.cooldown_per_token_secs).await?;
        Ok(())
    }

    pub async fn is_on_cooldown(&self, mint: &str) -> anyhow::Result<bool> {
        let key = self.key(&format!("cooldown:{}", mint));
        let mut conn = self.pool.clone();
        let exists: bool = conn.exists(&key).await?;
        Ok(exists)
    }

    pub async fn publish_trade_event(&self, event_json: &str) -> anyhow::Result<()> {
        let channel = self.key("events");
        let mut conn = self.pool.clone();
        conn.publish::<_, _, ()>(&channel, event_json).await?;
        Ok(())
    }
}
