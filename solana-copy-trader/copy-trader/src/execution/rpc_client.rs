use crate::config::{AppConfig, ConfirmStrategy};
use orbitflare_sdk::{RpcClient, RpcClientBuilder, WsClientBuilder};
use serde_json::json;
use std::sync::Arc;

pub struct SolanaRpcClient {
    rpc: RpcClient,
    config: Arc<AppConfig>,
    sim_latency: prometheus::Histogram,
}

impl SolanaRpcClient {
    pub fn new(config: Arc<AppConfig>, sim_latency: prometheus::Histogram) -> Self {
        let rpc = RpcClientBuilder::new()
            .url(&config.rpc.url)
            .commitment(&config.rpc.commitment)
            .timeout(std::time::Duration::from_secs(config.rpc.timeout_secs))
            .build()
            .expect("Failed to build RPC client");

        Self {
            rpc,
            config,
            sim_latency,
        }
    }

    pub async fn simulate_transaction(&self, tx_base64: &str) -> anyhow::Result<SimulationResult> {
        let start = std::time::Instant::now();

        let result = self
            .rpc
            .request(
                "simulateTransaction",
                json!([
                    tx_base64,
                    {
                        "encoding": "base64",
                        "commitment": self.config.rpc.commitment,
                        "replaceRecentBlockhash": true
                    }
                ]),
            )
            .await?;

        let elapsed_ms = start.elapsed().as_millis() as f64;
        self.sim_latency.observe(elapsed_ms);

        let value = &result["value"];
        if let Some(err) = value.get("err") {
            if !err.is_null() {
                let logs = value
                    .get("logs")
                    .and_then(|l| l.as_array())
                    .map(|l| {
                        l.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                return Ok(SimulationResult {
                    success: false,
                    error: Some(format!("{}", err)),
                    logs,
                    units_consumed: value.get("unitsConsumed").and_then(|u| u.as_u64()),
                });
            }
        }

        Ok(SimulationResult {
            success: true,
            error: None,
            logs: vec![],
            units_consumed: value.get("unitsConsumed").and_then(|u| u.as_u64()),
        })
    }

    pub async fn send_transaction(&self, tx_base64: &str) -> anyhow::Result<String> {
        let result = self
            .rpc
            .request(
                "sendTransaction",
                json!([
                    tx_base64,
                    {
                        "encoding": "base64",
                        "skipPreflight": false,
                        "preflightCommitment": self.config.rpc.commitment,
                        "maxRetries": self.config.rpc.max_retries
                    }
                ]),
            )
            .await?;

        let sig = result
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing signature in sendTransaction response"))?;

        Ok(sig.to_string())
    }

    pub async fn confirm_transaction(&self, signature: &str) -> anyhow::Result<bool> {
        match self.config.execution.confirmation.strategy {
            ConfirmStrategy::Websocket => self.confirm_via_websocket(signature).await,
            ConfirmStrategy::Poll => self.confirm_via_poll(signature).await,
        }
    }

    async fn confirm_via_poll(&self, signature: &str) -> anyhow::Result<bool> {
        let timeout =
            std::time::Duration::from_secs(self.config.execution.confirmation.timeout_secs);
        let poll_interval =
            std::time::Duration::from_millis(self.config.execution.confirmation.poll_interval_ms);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Ok(false);
            }

            let result = self
                .rpc
                .request(
                    "getSignatureStatuses",
                    json!([[signature], { "searchTransactionHistory": false }]),
                )
                .await?;

            if let Some(statuses) = result["value"].as_array() {
                if let Some(status) = statuses.first() {
                    if !status.is_null() {
                        match status.get("err") {
                            None => return Ok(true),
                            Some(err) if err.is_null() => return Ok(true),
                            Some(err) => anyhow::bail!("Transaction failed: {}", err),
                        }
                    }
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    async fn confirm_via_websocket(&self, signature: &str) -> anyhow::Result<bool> {
        let timeout =
            std::time::Duration::from_secs(self.config.execution.confirmation.timeout_secs);

        let ws_url = self
            .config
            .rpc
            .url
            .replace("https://", "wss://")
            .replace("http://", "ws://");

        let ws = WsClientBuilder::new().url(&ws_url).build().await?;

        let result = tokio::time::timeout(timeout, async {
            let mut sub = ws
                .signature_subscribe(signature, &self.config.rpc.commitment)
                .await?;
            let Some(value) = sub.next().await else {
                return Ok(false);
            };
            if let Some(err) = value.get("err") {
                if !err.is_null() {
                    anyhow::bail!("Transaction failed: {}", err);
                }
            }
            Ok(true)
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(_) => {
                tracing::warn!(
                    signature,
                    "WebSocket confirmation timed out, doing final poll check"
                );
                self.confirm_via_poll(signature).await
            }
        }
    }

    pub async fn get_token_account_mint(
        &self,
        token_account: &str,
    ) -> anyhow::Result<Option<String>> {
        let result = self
            .rpc
            .request(
                "getAccountInfo",
                json!([
                    token_account,
                    {
                        "encoding": "jsonParsed",
                        "commitment": self.config.rpc.commitment
                    }
                ]),
            )
            .await?;

        let mint = result
            .get("value")
            .and_then(|v| {
                if v.is_null() {
                    return None;
                }
                v.get("data")
            })
            .and_then(|d| d.get("parsed"))
            .and_then(|p| p.get("info"))
            .and_then(|i| i.get("mint"))
            .and_then(|m| m.as_str())
            .map(|s| s.to_string());

        Ok(mint)
    }

    pub async fn get_multiple_token_account_mints(
        &self,
        token_accounts: &[&str],
    ) -> anyhow::Result<Vec<Option<String>>> {
        if token_accounts.is_empty() {
            return Ok(vec![]);
        }

        let result = self
            .rpc
            .request(
                "getMultipleAccounts",
                json!([
                    token_accounts,
                    {
                        "encoding": "jsonParsed",
                        "commitment": self.config.rpc.commitment
                    }
                ]),
            )
            .await?;

        let Some(accounts) = result.get("value").and_then(|v| v.as_array()) else {
            return Ok(vec![None; token_accounts.len()]);
        };

        let mints: Vec<Option<String>> = accounts
            .iter()
            .map(|acct| {
                if acct.is_null() {
                    return None;
                }
                acct.get("data")
                    .and_then(|d| d.get("parsed"))
                    .and_then(|p| p.get("info"))
                    .and_then(|i| i.get("mint"))
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        Ok(mints)
    }

    pub async fn get_latest_blockhash(&self) -> anyhow::Result<solana_sdk::hash::Hash> {
        let (blockhash_str, _) = self.rpc.get_latest_blockhash().await?;
        let blockhash: solana_sdk::hash::Hash = blockhash_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid blockhash: {}", e))?;
        Ok(blockhash)
    }

    pub async fn get_balance(&self, pubkey: &str) -> anyhow::Result<u64> {
        Ok(self.rpc.get_balance(pubkey).await?)
    }
}

#[derive(Debug)]
pub struct SimulationResult {
    pub success: bool,
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub units_consumed: Option<u64>,
}
