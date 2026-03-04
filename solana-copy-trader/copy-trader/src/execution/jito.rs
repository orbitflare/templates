use crate::config::AppConfig;
#[allow(deprecated)]
use solana_sdk::system_instruction;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

const JITO_TIP_ACCOUNTS: &[&str] = &[
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4bVqkfRtQ7NmXwkiNPNYFNY",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSLa2f4mC7Q68UDy5dg",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

pub struct JitoClient {
    http: reqwest::Client,
    config: Arc<AppConfig>,
}

impl JitoClient {
    pub fn new(config: Arc<AppConfig>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self { http, config }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.execution.jito.enabled
    }

    fn build_tip_transaction(
        &self,
        keypair: &Keypair,
        recent_blockhash: solana_sdk::hash::Hash,
    ) -> Transaction {
        let tip_account_str =
            JITO_TIP_ACCOUNTS[fastrand::usize(..JITO_TIP_ACCOUNTS.len())];
        let tip_account: Pubkey = tip_account_str.parse().unwrap();

        let instruction = system_instruction::transfer(
            &keypair.pubkey(),
            &tip_account,
            self.config.execution.jito.tip_lamports,
        );

        let mut tx = Transaction::new_with_payer(&[instruction], Some(&keypair.pubkey()));
        tx.sign(&[keypair], recent_blockhash);
        tx
    }

    pub async fn send_bundle(
        &self,
        swap_tx_base64: &str,
        keypair: &Keypair,
        recent_blockhash: solana_sdk::hash::Hash,
    ) -> anyhow::Result<String> {
        if !self.config.execution.jito.enabled {
            anyhow::bail!("Jito is not enabled");
        }

        let url = format!("{}/api/v1/bundles", self.config.execution.jito.block_engine_url);

        let tip_tx = self.build_tip_transaction(keypair, recent_blockhash);
        let tip_tx_bytes = bincode::serialize(&tip_tx)
            .map_err(|e| anyhow::anyhow!("Failed to serialize tip tx: {}", e))?;
        let tip_tx_base64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &tip_tx_bytes,
        );

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendBundle",
            "params": [[swap_tx_base64, tip_tx_base64]]
        });

        let resp = self.http.post(&url).json(&body).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jito bundle submission error {}: {}", status, body);
        }

        let json: serde_json::Value = resp.json().await?;

        if let Some(error) = json.get("error") {
            anyhow::bail!("Jito error: {}", error);
        }

        let bundle_id = json["result"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        tracing::info!(
            bundle_id = %bundle_id,
            tip_lamports = self.config.execution.jito.tip_lamports,
            "Bundle submitted to Jito (swap + tip)"
        );

        Ok(bundle_id)
    }
}
