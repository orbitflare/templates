use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use strum::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum Dex {
    #[strum(serialize = "jupiter_v6")]
    JupiterV6,
    #[strum(serialize = "raydium_amm")]
    RaydiumAmm,
    #[strum(serialize = "raydium_cpmm")]
    RaydiumCpmm,
    #[strum(serialize = "pumpfun")]
    PumpFun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum Direction {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TradeStatus {
    Detected,
    Filtered,
    Simulated,
    Submitted,
    Confirmed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct TradeIntent {
    pub wallet: Pubkey,
    pub wallet_label: Option<String>,
    pub target_tx_signature: String,
    pub slot: u64,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub input_amount: u64,
    pub output_amount: Option<u64>,
    pub slippage_bps: Option<u32>,
    pub direction: Direction,
    pub dex: Dex,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub target_wallet: String,
    pub target_label: Option<String>,
    pub target_tx_sig: String,
    pub direction: Direction,
    pub dex: Dex,
    pub input_mint: String,
    pub output_mint: String,
    pub target_amount: f64,
    pub our_amount_sol: f64,
    pub our_tx_sig: Option<String>,
    pub status: TradeStatus,
    pub failure_reason: Option<String>,
    pub slippage_bps: Option<i32>,
    pub priority_fee: Option<i64>,
    pub latency_ms: Option<i32>,
    pub dry_run: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterQuote {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: Option<String>,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterSwapResponse {
    #[serde(rename = "swapTransaction")]
    pub swap_transaction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub mint: String,
    pub entry_amount_sol: f64,
    pub entry_tx_sig: String,
    pub opened_at: DateTime<Utc>,
}
