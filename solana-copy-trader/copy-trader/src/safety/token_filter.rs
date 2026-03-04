use crate::config::AppConfig;
use crate::safety::SafetyError;
use crate::types::TradeIntent;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashSet;
use std::sync::Arc;

pub struct TokenFilter {
    blacklist: HashSet<Pubkey>,
    config: Arc<AppConfig>,
    http: reqwest::Client,
}

impl TokenFilter {
    pub fn new(config: Arc<AppConfig>) -> Self {
        let blacklist: HashSet<Pubkey> = config
            .safety
            .token_filters
            .blacklisted_tokens
            .iter()
            .filter_map(|s| s.parse::<Pubkey>().ok())
            .collect();

        if !blacklist.is_empty() {
            tracing::info!(count = blacklist.len(), "Token blacklist loaded");
        }

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.rpc.timeout_secs))
            .build()
            .expect("Failed to create HTTP client for token filter");

        Self {
            blacklist,
            config,
            http,
        }
    }

    pub async fn check(&self, intent: &TradeIntent) -> Result<(), SafetyError> {
        if self.blacklist.contains(&intent.output_mint) {
            return Err(SafetyError::Blacklisted(intent.output_mint.to_string()));
        }
        if self.blacklist.contains(&intent.input_mint) {
            return Err(SafetyError::Blacklisted(intent.input_mint.to_string()));
        }

        let token_to_check = intent.output_mint.to_string();
        let filters = &self.config.safety.token_filters;
        let needs_mint_info = filters.require_mint_renounced
            || filters.require_freeze_renounced
            || filters.min_token_age_secs > 0;

        if needs_mint_info {
            if let Some(mint_info) = self.fetch_mint_info(&token_to_check).await {
                if filters.require_mint_renounced {
                    if let Some(ref authority) = mint_info.mint_authority {
                        if !authority.is_empty() {
                            return Err(SafetyError::MintAuthority(token_to_check.clone()));
                        }
                    }
                }

                if filters.require_freeze_renounced {
                    if let Some(ref authority) = mint_info.freeze_authority {
                        if !authority.is_empty() {
                            return Err(SafetyError::FreezeAuthority(token_to_check.clone()));
                        }
                    }
                }
            } else {
                tracing::warn!(
                    mint = %token_to_check,
                    "Could not fetch mint info for safety check, allowing trade"
                );
            }
        }

        if filters.min_token_age_secs > 0 {
            match self.fetch_account_age(&token_to_check).await {
                Some(age_secs) => {
                    if age_secs < filters.min_token_age_secs {
                        return Err(SafetyError::TokenTooNew {
                            mint: token_to_check.clone(),
                            age_secs,
                            min_secs: filters.min_token_age_secs,
                        });
                    }
                }
                None => {
                    tracing::warn!(
                        mint = %token_to_check,
                        "Could not determine token age, allowing trade"
                    );
                }
            }
        }

        if filters.min_liquidity_sol > 0.0 {
            tracing::trace!(
                mint = %token_to_check,
                min_liquidity = filters.min_liquidity_sol,
                "Liquidity check deferred to Jupiter quote"
            );
        }

        Ok(())
    }

    async fn fetch_mint_info(&self, mint: &str) -> Option<MintInfo> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                mint,
                {
                    "encoding": "jsonParsed",
                    "commitment": self.config.rpc.commitment
                }
            ]
        });

        let resp = self
            .http
            .post(&self.config.rpc.url)
            .json(&body)
            .send()
            .await
            .ok()?;

        let json: serde_json::Value = resp.json().await.ok()?;

        let info = json
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| {
                if v.is_null() {
                    return None;
                }
                v.get("data")
            })
            .and_then(|d| d.get("parsed"))
            .and_then(|p| p.get("info"))?;

        let mint_authority = info
            .get("mintAuthority")
            .and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    v.as_str().map(|s| s.to_string())
                }
            });

        let freeze_authority = info
            .get("freezeAuthority")
            .and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    v.as_str().map(|s| s.to_string())
                }
            });

        Some(MintInfo {
            mint_authority,
            freeze_authority,
        })
    }

    async fn fetch_account_age(&self, mint: &str) -> Option<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignaturesForAddress",
            "params": [
                mint,
                {
                    "limit": 1,
                    "commitment": "finalized"
                }
            ]
        });

        let resp = self
            .http
            .post(&self.config.rpc.url)
            .json(&body)
            .send()
            .await
            .ok()?;

        let json: serde_json::Value = resp.json().await.ok()?;

        let results = json.get("result")?.as_array()?;
        if results.is_empty() {
            return None;
        }

        let oldest = results.last()?;
        let block_time = oldest.get("blockTime")?.as_i64()?;

        let now = chrono::Utc::now().timestamp();
        let age = (now - block_time).max(0) as u64;

        Some(age)
    }
}

struct MintInfo {
    mint_authority: Option<String>,
    freeze_authority: Option<String>,
}
