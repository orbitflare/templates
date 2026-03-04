use crate::config::AppConfig;
use crate::execution::jito::JitoClient;
use crate::execution::jupiter_client::JupiterClient;
use crate::execution::rpc_client::SolanaRpcClient;
use crate::execution::sizing::PositionSizer;
use crate::output::metrics::Metrics;
use crate::safety::SafetyFilter;
use crate::state::redis::RedisClient;
use crate::types::{Direction, Position, TradeIntent, TradeRecord, TradeStatus};
use chrono::Utc;
use solana_sdk::signature::{Keypair, Signer};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub struct ExecutionEngine {
    config: Arc<AppConfig>,
    safety: SafetyFilter,
    sizer: PositionSizer,
    jupiter: JupiterClient,
    rpc: SolanaRpcClient,
    jito: JitoClient,
    redis: Arc<RedisClient>,
    keypair: Option<Arc<Keypair>>,
    metrics: Arc<Metrics>,
    intent_rx: mpsc::Receiver<TradeIntent>,
    record_tx: mpsc::Sender<TradeRecord>,
    shutdown_rx: watch::Receiver<bool>,
}

impl ExecutionEngine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Arc<AppConfig>,
        redis: Arc<RedisClient>,
        keypair: Option<Arc<Keypair>>,
        metrics: Arc<Metrics>,
        intent_rx: mpsc::Receiver<TradeIntent>,
        record_tx: mpsc::Sender<TradeRecord>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        let safety = SafetyFilter::new(config.clone(), redis.clone());
        let sizer = PositionSizer::new(config.clone());
        let jupiter = JupiterClient::new(
            config.clone(),
            redis.clone(),
            metrics.jupiter_cache_hits.clone(),
        );
        let rpc = SolanaRpcClient::new(config.clone(), metrics.simulation_latency.clone());
        let jito = JitoClient::new(config.clone());

        Self {
            config,
            safety,
            sizer,
            jupiter,
            rpc,
            jito,
            redis,
            keypair,
            metrics,
            intent_rx,
            record_tx,
            shutdown_rx,
        }
    }

    pub async fn run(mut self) {
        tracing::info!(
            dry_run = self.config.execution.dry_run,
            "ExecutionEngine started"
        );

        loop {
            tokio::select! {
                Some(intent) = self.intent_rx.recv() => {
                    self.process_intent(intent).await;
                }
                _ = self.shutdown_rx.changed() => {
                    tracing::info!("ExecutionEngine shutting down");
                    break;
                }
            }
        }
    }

    async fn process_intent(&self, intent: TradeIntent) {
        let start = std::time::Instant::now();
        let target_wallet = intent.wallet.to_string();
        let dex_label = intent.dex.to_string();

        tracing::info!(
            wallet = %intent.wallet,
            label = ?intent.wallet_label,
            dex = %intent.dex,
            direction = %intent.direction,
            input_mint = %intent.input_mint,
            output_mint = %intent.output_mint,
            input_amount = intent.input_amount,
            tx_sig = %intent.target_tx_signature,
            "Processing trade intent"
        );

        if let Err(e) = self.safety.check(&intent).await {
            tracing::warn!(
                reason = %e,
                wallet = %intent.wallet,
                "Trade filtered by safety check"
            );
            self.metrics
                .trades_total
                .with_label_values(&[target_wallet.as_str(), "filtered", dex_label.as_str()])
                .inc();

            let record = self.build_record(&intent, TradeStatus::Filtered, Some(e.to_string()), None, &target_wallet, start);
            let _ = self.record_tx.send(record).await;
            return;
        }

        let Some(trade_lamports) = self.sizer.calculate(&intent) else {
            tracing::debug!("Trade size below minimum, skipping");
            return;
        };

        let trade_sol = trade_lamports as f64 / 1e9;

        if self.config.execution.dry_run {
            tracing::info!(
                wallet = %intent.wallet,
                dex = %intent.dex,
                direction = %intent.direction,
                output_mint = %intent.output_mint,
                trade_sol,
                "[DRY RUN] Would copy trade"
            );

            self.metrics
                .trades_total
                .with_label_values(&[target_wallet.as_str(), "simulated", dex_label.as_str()])
                .inc();

            let record = self.build_record(&intent, TradeStatus::Simulated, None, None, &target_wallet, start);
            let _ = self.record_tx.send(record).await;

            let _ = self.redis.set_cooldown(&intent.output_mint.to_string()).await;
            let _ = self.redis.increment_trade_count().await;
            return;
        }

        let wallet_pubkey = match &self.keypair {
            Some(kp) => kp.pubkey().to_string(),
            None => {
                tracing::error!("No keypair loaded for live trading");
                let record = self.build_record(
                    &intent,
                    TradeStatus::Failed,
                    Some("No keypair loaded".to_string()),
                    None,
                    &target_wallet,
                    start,
                );
                let _ = self.record_tx.send(record).await;
                return;
            }
        };

        let slippage = intent
            .slippage_bps
            .unwrap_or(self.config.execution.slippage.default_bps)
            .min(self.config.execution.slippage.max_bps);

        let input_mint_str = intent.input_mint.to_string();
        let output_mint_str = intent.output_mint.to_string();

        let quote = match self
            .jupiter
            .get_quote(&input_mint_str, &output_mint_str, trade_lamports, slippage)
            .await
        {
            Ok(q) => q,
            Err(e) => {
                tracing::error!(error = %e, "Jupiter quote failed");
                self.metrics
                    .trades_total
                    .with_label_values(&[target_wallet.as_str(), "failed", dex_label.as_str()])
                    .inc();
                let record = self.build_record(
                    &intent,
                    TradeStatus::Failed,
                    Some(format!("Quote failed: {}", e)),
                    None,
                    &target_wallet,
                    start,
                );
                let _ = self.record_tx.send(record).await;
                return;
            }
        };

        let swap_resp = match self.jupiter.get_swap_transaction(&quote, &wallet_pubkey).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Jupiter swap tx build failed");
                let record = self.build_record(
                    &intent,
                    TradeStatus::Failed,
                    Some(format!("Swap build failed: {}", e)),
                    None,
                    &target_wallet,
                    start,
                );
                let _ = self.record_tx.send(record).await;
                return;
            }
        };

        let sim_result = match self.rpc.simulate_transaction(&swap_resp.swap_transaction).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "Simulation RPC call failed");
                let record = self.build_record(
                    &intent,
                    TradeStatus::Failed,
                    Some(format!("Simulation error: {}", e)),
                    None,
                    &target_wallet,
                    start,
                );
                let _ = self.record_tx.send(record).await;
                return;
            }
        };

        if !sim_result.success {
            tracing::warn!(
                error = ?sim_result.error,
                "Simulation failed"
            );
            self.metrics
                .trades_total
                .with_label_values(&[target_wallet.as_str(), "failed", dex_label.as_str()])
                .inc();
            let record = self.build_record(
                &intent,
                TradeStatus::Failed,
                Some(format!("Simulation failed: {:?}", sim_result.error)),
                None,
                &target_wallet,
                start,
            );
            let _ = self.record_tx.send(record).await;
            return;
        }

        tracing::info!(
            units_consumed = ?sim_result.units_consumed,
            "Simulation passed"
        );

        let send_result = if self.jito.is_enabled() {
            match &self.keypair {
                Some(kp) => {
                    match self.rpc.get_latest_blockhash().await {
                        Ok(blockhash) => {
                            self.jito
                                .send_bundle(&swap_resp.swap_transaction, kp, blockhash)
                                .await
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to get blockhash for Jito tip: {}", e)),
                    }
                }
                None => Err(anyhow::anyhow!("No keypair available for Jito tip")),
            }
        } else {
            self.rpc.send_transaction(&swap_resp.swap_transaction).await
        };

        let our_tx_sig = match send_result {
            Ok(sig) => {
                tracing::info!(signature = %sig, "Transaction submitted");
                self.metrics
                    .trades_total
                    .with_label_values(&[target_wallet.as_str(), "submitted", dex_label.as_str()])
                    .inc();
                sig
            }
            Err(e) => {
                tracing::error!(error = %e, "Transaction submission failed");
                self.metrics
                    .trades_total
                    .with_label_values(&[target_wallet.as_str(), "failed", dex_label.as_str()])
                    .inc();
                let record = self.build_record(
                    &intent,
                    TradeStatus::Failed,
                    Some(format!("Send failed: {}", e)),
                    None,
                    &target_wallet,
                    start,
                );
                let _ = self.record_tx.send(record).await;
                return;
            }
        };

        match self.rpc.confirm_transaction(&our_tx_sig).await {
            Ok(true) => {
                let latency_ms = start.elapsed().as_millis() as f64;
                tracing::info!(
                    signature = %our_tx_sig,
                    latency_ms,
                    "Transaction confirmed"
                );
                self.metrics
                    .trades_total
                    .with_label_values(&[target_wallet.as_str(), "confirmed", dex_label.as_str()])
                    .inc();
                self.metrics
                    .trade_latency
                    .with_label_values(&[target_wallet.as_str(), dex_label.as_str()])
                    .observe(latency_ms);

                self.metrics
                    .slippage_bps
                    .with_label_values(&[dex_label.as_str()])
                    .observe(slippage as f64);

                if intent.direction == Direction::Buy {
                    let position = Position {
                        mint: intent.output_mint.to_string(),
                        entry_amount_sol: trade_sol,
                        entry_tx_sig: our_tx_sig.clone(),
                        opened_at: Utc::now(),
                    };
                    let _ = self.redis.add_position(&position).await;
                    self.metrics.open_positions.inc();
                    self.metrics.portfolio_exposure.add(trade_sol);
                } else {
                    let _ = self.redis.remove_position(&intent.input_mint.to_string()).await;
                    self.metrics.open_positions.dec();
                    self.metrics.portfolio_exposure.sub(trade_sol);
                }

                let record = self.build_record(
                    &intent,
                    TradeStatus::Confirmed,
                    None,
                    Some(our_tx_sig),
                    &target_wallet,
                    start,
                );
                let _ = self.record_tx.send(record).await;
            }
            Ok(false) => {
                tracing::warn!(signature = %our_tx_sig, "Transaction confirmation timed out");
                let record = self.build_record(
                    &intent,
                    TradeStatus::Failed,
                    Some("Confirmation timeout".to_string()),
                    Some(our_tx_sig),
                    &target_wallet,
                    start,
                );
                let _ = self.record_tx.send(record).await;
            }
            Err(e) => {
                tracing::error!(error = %e, "Confirmation check failed");
                let record = self.build_record(
                    &intent,
                    TradeStatus::Failed,
                    Some(format!("Confirmation error: {}", e)),
                    Some(our_tx_sig),
                    &target_wallet,
                    start,
                );
                let _ = self.record_tx.send(record).await;
            }
        }

        let _ = self.redis.set_cooldown(&intent.output_mint.to_string()).await;
        let _ = self.redis.increment_trade_count().await;
    }

    fn build_record(
        &self,
        intent: &TradeIntent,
        status: TradeStatus,
        failure_reason: Option<String>,
        our_tx_sig: Option<String>,
        target_wallet: &str,
        start: std::time::Instant,
    ) -> TradeRecord {
        let latency_ms = start.elapsed().as_millis() as i32;

        TradeRecord {
            target_wallet: target_wallet.to_string(),
            target_label: intent.wallet_label.clone(),
            target_tx_sig: intent.target_tx_signature.clone(),
            direction: intent.direction,
            dex: intent.dex,
            input_mint: intent.input_mint.to_string(),
            output_mint: intent.output_mint.to_string(),
            target_amount: intent.input_amount as f64 / 1e9,
            our_amount_sol: self
                .sizer
                .calculate(intent)
                .map(|l| l as f64 / 1e9)
                .unwrap_or(0.0),
            our_tx_sig,
            status,
            failure_reason,
            slippage_bps: intent.slippage_bps.map(|s| s as i32),
            priority_fee: None,
            latency_ms: Some(latency_ms),
            dry_run: self.config.execution.dry_run,
            created_at: Utc::now(),
        }
    }
}
