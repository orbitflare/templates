use crate::config::{AppConfig, SizingMode};
use crate::types::TradeIntent;
use std::sync::Arc;

pub struct PositionSizer {
    config: Arc<AppConfig>,
}

impl PositionSizer {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }

    pub fn calculate(&self, intent: &TradeIntent) -> Option<u64> {
        let wallet_str = intent.wallet.to_string();

        let override_config = self
            .config
            .targets
            .iter()
            .find(|t| t.address == wallet_str)
            .and_then(|t| t.sizing.as_ref());

        let (mode, fixed_sol, proportion) = if let Some(ovr) = override_config {
            (
                &ovr.mode,
                ovr.fixed_amount_sol.unwrap_or(self.config.execution.sizing.fixed_amount_sol),
                ovr.proportion.unwrap_or(self.config.execution.sizing.proportion),
            )
        } else {
            (
                &self.config.execution.sizing.mode,
                self.config.execution.sizing.fixed_amount_sol,
                self.config.execution.sizing.proportion,
            )
        };

        let amount_sol = match mode {
            SizingMode::Fixed => fixed_sol,
            SizingMode::Proportional => {
                let target_sol = intent.input_amount as f64 / 1e9;
                target_sol * proportion
            }
            SizingMode::Mirror => {
                intent.input_amount as f64 / 1e9
            }
        };

        let max = self.config.execution.sizing.max_trade_sol;
        let min = self.config.execution.sizing.min_trade_sol;

        let capped = amount_sol.min(max);

        if capped < min {
            tracing::debug!(
                amount_sol = capped,
                min_trade_sol = min,
                "Trade size below minimum, skipping"
            );
            return None;
        }

        let lamports = (capped * 1e9) as u64;

        tracing::info!(
            mode = ?mode,
            target_amount_lamports = intent.input_amount,
            our_amount_sol = capped,
            our_amount_lamports = lamports,
            "Position sized"
        );

        Some(lamports)
    }
}
