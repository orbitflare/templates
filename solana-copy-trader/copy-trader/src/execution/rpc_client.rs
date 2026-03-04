use crate::config::{AppConfig, ConfirmStrategy};
use std::sync::Arc;

pub struct SolanaRpcClient {
    http: reqwest::Client,
    config: Arc<AppConfig>,
    sim_latency: prometheus::Histogram,
}

impl SolanaRpcClient {
    pub fn new(config: Arc<AppConfig>, sim_latency: prometheus::Histogram) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.rpc.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http,
            config,
            sim_latency,
        }
    }

    async fn rpc_call(&self, method: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });
        let resp = self.http.post(&self.config.rpc.url).json(&body).send().await?;
        let json: serde_json::Value = resp.json().await?;
        if let Some(error) = json.get("error") {
            anyhow::bail!("RPC error in {}: {}", method, error);
        }
        Ok(json)
    }

    pub async fn simulate_transaction(&self, tx_base64: &str) -> anyhow::Result<SimulationResult> {
        let start = std::time::Instant::now();

        let json = self.rpc_call("simulateTransaction", serde_json::json!([
            tx_base64,
            {
                "encoding": "base64",
                "commitment": self.config.rpc.commitment,
                "replaceRecentBlockhash": true
            }
        ])).await?;

        let elapsed_ms = start.elapsed().as_millis() as f64;
        self.sim_latency.observe(elapsed_ms);

        let result = &json["result"]["value"];
        if let Some(err) = result.get("err") {
            if !err.is_null() {
                let logs = result
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
                    units_consumed: result.get("unitsConsumed").and_then(|u| u.as_u64()),
                });
            }
        }

        let units_consumed = result.get("unitsConsumed").and_then(|u| u.as_u64());

        Ok(SimulationResult {
            success: true,
            error: None,
            logs: vec![],
            units_consumed,
        })
    }

    pub async fn send_transaction(&self, tx_base64: &str) -> anyhow::Result<String> {
        let json = self.rpc_call("sendTransaction", serde_json::json!([
            tx_base64,
            {
                "encoding": "base64",
                "skipPreflight": false,
                "preflightCommitment": self.config.rpc.commitment,
                "maxRetries": self.config.rpc.max_retries
            }
        ])).await?;

        let sig = json["result"]
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
        let timeout = std::time::Duration::from_secs(self.config.execution.confirmation.timeout_secs);
        let poll_interval = std::time::Duration::from_millis(
            self.config.execution.confirmation.poll_interval_ms,
        );
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Ok(false);
            }

            let json = self.rpc_call("getSignatureStatuses", serde_json::json!([
                [signature],
                { "searchTransactionHistory": false }
            ])).await?;

            if let Some(statuses) = json["result"]["value"].as_array() {
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
        use futures::SinkExt;
        use tokio_stream::StreamExt;
        use tokio_tungstenite::tungstenite::Message;

        let timeout = std::time::Duration::from_secs(self.config.execution.confirmation.timeout_secs);

        let ws_url = self
            .config
            .rpc
            .url
            .replace("https://", "wss://")
            .replace("http://", "ws://");

        let (mut ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| anyhow::anyhow!("WebSocket connection failed: {}", e))?;

        let subscribe_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "signatureSubscribe",
            "params": [
                signature,
                {
                    "commitment": self.config.rpc.commitment,
                    "enableReceivedNotification": false
                }
            ]
        });

        ws_stream
            .send(Message::Text(subscribe_msg.to_string().into()))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send subscribe message: {}", e))?;

        let result = tokio::time::timeout(timeout, async {
            while let Some(msg) = ws_stream.next().await {
                let msg = msg.map_err(|e| anyhow::anyhow!("WebSocket error: {}", e))?;

                if let Message::Text(text) = msg {
                    let json: serde_json::Value = serde_json::from_str(&text)
                        .unwrap_or(serde_json::Value::Null);

                    if json.get("method").and_then(|m| m.as_str()) == Some("signatureNotification") {
                        let result = &json["params"]["result"];
                        if let Some(err) = result.get("err") {
                            if !err.is_null() {
                                anyhow::bail!("Transaction failed: {}", err);
                            }
                        }
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })
        .await;

        let _ = ws_stream.close(None).await;

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

    pub async fn get_token_account_mint(&self, token_account: &str) -> anyhow::Result<Option<String>> {
        let json = self.rpc_call("getAccountInfo", serde_json::json!([
            token_account,
            {
                "encoding": "jsonParsed",
                "commitment": self.config.rpc.commitment
            }
        ])).await?;

        let mint = json
            .get("result")
            .and_then(|r| r.get("value"))
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

        let json = self.rpc_call("getMultipleAccounts", serde_json::json!([
            token_accounts,
            {
                "encoding": "jsonParsed",
                "commitment": self.config.rpc.commitment
            }
        ])).await?;

        let accounts = json
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_array());

        let Some(accounts) = accounts else {
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
        let json = self.rpc_call("getLatestBlockhash", serde_json::json!([
            {"commitment": self.config.rpc.commitment}
        ])).await?;

        let blockhash_str = json["result"]["value"]["blockhash"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing blockhash in response"))?;

        let blockhash: solana_sdk::hash::Hash = blockhash_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid blockhash: {}", e))?;

        Ok(blockhash)
    }

    pub async fn get_balance(&self, pubkey: &str) -> anyhow::Result<u64> {
        let json = self.rpc_call("getBalance", serde_json::json!([
            pubkey, {"commitment": self.config.rpc.commitment}
        ])).await?;

        let balance = json["result"]["value"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse balance"))?;

        Ok(balance)
    }
}

#[derive(Debug)]
pub struct SimulationResult {
    pub success: bool,
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub units_consumed: Option<u64>,
}
