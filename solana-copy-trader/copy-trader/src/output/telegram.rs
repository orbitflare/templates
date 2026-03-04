use crate::config::AppConfig;
use crate::types::{TradeRecord, TradeStatus};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub struct TelegramNotifier {
    http: reqwest::Client,
    bot_token: String,
    chat_id: String,
    notify_on: Vec<String>,
}

impl TelegramNotifier {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self {
            http: reqwest::Client::new(),
            bot_token: config.notifications.telegram.bot_token.clone(),
            chat_id: config.notifications.telegram.chat_id.clone(),
            notify_on: config.notifications.telegram.notify_on.clone(),
        }
    }

    pub async fn run(
        self,
        mut record_rx: mpsc::Receiver<TradeRecord>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        tracing::info!("Telegram notifier started");

        loop {
            tokio::select! {
                Some(record) = record_rx.recv() => {
                    if self.should_notify(&record) {
                        if let Err(e) = self.send_notification(&record).await {
                            tracing::warn!(error = %e, "Failed to send Telegram notification");
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    tracing::info!("Telegram notifier shutting down");
                    return;
                }
            }
        }
    }

    fn should_notify(&self, record: &TradeRecord) -> bool {
        let event = match record.status {
            TradeStatus::Confirmed => "trade_executed",
            TradeStatus::Failed => "trade_failed",
            TradeStatus::Filtered => "safety_triggered",
            _ => return false,
        };
        self.notify_on.iter().any(|n| n == event)
    }

    async fn send_notification(&self, record: &TradeRecord) -> anyhow::Result<()> {
        let emoji = match record.status {
            TradeStatus::Confirmed => "✅",
            TradeStatus::Failed => "❌",
            TradeStatus::Filtered => "🛡️",
            _ => "ℹ️",
        };

        let dry_tag = if record.dry_run { " [DRY RUN]" } else { "" };
        let label = record
            .target_label
            .as_deref()
            .unwrap_or(&record.target_wallet[..8]);

        let text = format!(
            "{emoji}{dry_tag} *{status}*\n\
             Target: `{label}`\n\
             DEX: {dex} | Direction: {direction}\n\
             Token: `{output_mint}`\n\
             Amount: {our_sol:.4} SOL\n\
             {extra}",
            status = record.status,
            dex = record.dex,
            direction = record.direction,
            output_mint = &record.output_mint[..16],
            our_sol = record.our_amount_sol,
            extra = if let Some(ref reason) = record.failure_reason {
                format!("Reason: {}\n", reason)
            } else if let Some(ref sig) = record.our_tx_sig {
                format!("TX: `{}`\n", sig)
            } else {
                String::new()
            },
        );

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        self.http
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown",
                "disable_web_page_preview": true
            }))
            .send()
            .await?;

        Ok(())
    }

    pub async fn send_system_message(&self, message: &str) -> anyhow::Result<()> {
        if !self.notify_on.iter().any(|n| n == "stream_reconnect") {
            return Ok(());
        }

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        self.http
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": format!("⚠️ {}", message),
                "parse_mode": "Markdown"
            }))
            .send()
            .await?;

        Ok(())
    }
}
