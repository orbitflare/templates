use crate::decoder::DexDecoder;
use crate::types::{Dex, Direction, TradeIntent};
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;

const BUY_DISC: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
const SELL_DISC: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

// Pump.fun account layout for buy/sell:
// 0: global state
// 1: fee recipient
// 2: mint
// 3: bonding curve
// 4: associated bonding curve
// 5: associated user
// 6: user (signer)
// 7: system program
// 8: token program
// 9: rent
// 10: event authority
// 11: program

const MINT_INDEX: usize = 2;

pub struct PumpFunDecoder {
    program_id: Pubkey,
}

impl PumpFunDecoder {
    pub fn new(program_id: Pubkey) -> Self {
        Self { program_id }
    }
}

impl DexDecoder for PumpFunDecoder {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn dex(&self) -> Dex {
        Dex::PumpFun
    }

    fn decode(
        &self,
        accounts: &[Pubkey],
        instruction_accounts: &[u8],
        data: &[u8],
        wallet: &Pubkey,
        wallet_label: Option<&str>,
        signature: &str,
        slot: u64,
    ) -> Option<TradeIntent> {
        if data.len() < 24 {
            return None;
        }

        let disc = &data[..8];

        let (direction, input_amount, output_amount) = if disc == BUY_DISC {
            let amount = u64::from_le_bytes(data[8..16].try_into().ok()?);
            let max_sol = u64::from_le_bytes(data[16..24].try_into().ok()?);
            (Direction::Buy, max_sol, Some(amount))
        } else if disc == SELL_DISC {
            let amount = u64::from_le_bytes(data[8..16].try_into().ok()?);
            let min_sol = u64::from_le_bytes(data[16..24].try_into().ok()?);
            (Direction::Sell, amount, Some(min_sol))
        } else {
            return None;
        };

        let mint_account_idx = instruction_accounts.get(MINT_INDEX).copied()? as usize;
        let mint = accounts.get(mint_account_idx).copied()?;

        let sol_mint: Pubkey = solana_sdk::system_program::ID;

        let (input_mint, output_mint) = match direction {
            Direction::Buy => (sol_mint, mint),
            Direction::Sell => (mint, sol_mint),
        };

        tracing::info!(
            dex = "pumpfun",
            direction = %direction,
            mint = %mint,
            input_amount,
            wallet = %wallet,
            signature,
            "Decoded Pump.fun swap"
        );

        Some(TradeIntent {
            wallet: *wallet,
            wallet_label: wallet_label.map(|s| s.to_string()),
            target_tx_signature: signature.to_string(),
            slot,
            input_mint,
            output_mint,
            input_amount,
            output_amount,
            slippage_bps: None,
            direction,
            dex: Dex::PumpFun,
            detected_at: Utc::now(),
        })
    }
}
