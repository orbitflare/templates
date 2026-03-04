use crate::config::{AppConfig, FeeStrategy};
use crate::state::redis::RedisClient;
use crate::types::{JupiterQuote, JupiterSwapResponse};
use std::sync::Arc;

const JUPITER_QUOTE_URL: &str = "https://quote-api.jup.ag/v6/quote";
const JUPITER_SWAP_URL: &str = "https://quote-api.jup.ag/v6/swap";

pub struct JupiterClient {
    http: reqwest::Client,
    config: Arc<AppConfig>,
    redis: Arc<RedisClient>,
    cache_hits: prometheus::IntCounter,
}

impl JupiterClient {
    pub fn new(
        config: Arc<AppConfig>,
        redis: Arc<RedisClient>,
        cache_hits: prometheus::IntCounter,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.rpc.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http,
            config,
            redis,
            cache_hits,
        }
    }

    pub async fn get_quote(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        slippage_bps: u32,
    ) -> anyhow::Result<JupiterQuote> {
        let cache_key = format!("{}:{}:{}", input_mint, output_mint, amount);
        if let Ok(Some(cached)) = self.redis.get_cached_quote(&cache_key).await {
            self.cache_hits.inc();
            let quote: JupiterQuote = serde_json::from_str(&cached)?;
            return Ok(quote);
        }

        let resp = self
            .http
            .get(JUPITER_QUOTE_URL)
            .query(&[
                ("inputMint", input_mint),
                ("outputMint", output_mint),
                ("amount", &amount.to_string()),
                ("slippageBps", &slippage_bps.to_string()),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jupiter quote API error {}: {}", status, body);
        }

        let quote: JupiterQuote = resp.json().await?;

        if let Ok(json) = serde_json::to_string(&quote) {
            let _ = self.redis.set_cached_quote(&cache_key, &json).await;
        }

        Ok(quote)
    }

    pub async fn get_swap_transaction(
        &self,
        quote: &JupiterQuote,
        user_public_key: &str,
    ) -> anyhow::Result<JupiterSwapResponse> {
        let fee_config = &self.config.execution.priority_fee;
        let priority_fee_value: serde_json::Value = match fee_config.strategy {
            FeeStrategy::Fixed => {
                let fee = fee_config.fixed_lamports.min(fee_config.max_lamports);
                serde_json::json!(fee)
            }
            FeeStrategy::Dynamic => {
                serde_json::json!({
                    "autoMultiplier": 1,
                    "maxLamports": fee_config.max_lamports
                })
            }
        };

        let body = serde_json::json!({
            "quoteResponse": quote,
            "userPublicKey": user_public_key,
            "wrapAndUnwrapSol": true,
            "dynamicComputeUnitLimit": true,
            "prioritizationFeeLamports": priority_fee_value
        });

        let resp = self
            .http
            .post(JUPITER_SWAP_URL)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jupiter swap API error {}: {}", status, body);
        }

        let swap_resp: JupiterSwapResponse = resp.json().await?;
        Ok(swap_resp)
    }
}
