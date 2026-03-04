use crate::decoder::DexDecoder;
use crate::types::{Dex, Direction, TradeIntent};
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;

// Jupiter v6 SharedAccountsRoute discriminator (Anchor-style: sha256("global:shared_accounts_route")[..8])
const SHARED_ACCOUNTS_ROUTE_DISC: [u8; 8] = [193, 32, 155, 51, 65, 214, 156, 129];
// SharedAccountsExactOutRoute
const SHARED_ACCOUNTS_EXACT_OUT_DISC: [u8; 8] = [176, 209, 105, 168, 154, 125, 69, 62];

// Jupiter v6 SharedAccountsRoute account layout:
// 0: token program
// 1: program authority
// 2: user transfer authority (signer)
// 3: source token account
// 4: program source token account
// 5: program destination token account
// 6: destination token account
// 7: source mint
// 8: destination mint
// ...remaining: route plan accounts

const SOURCE_MINT_INDEX: usize = 7;
const DEST_MINT_INDEX: usize = 8;

pub struct JupiterDecoder {
    program_id: Pubkey,
}

impl JupiterDecoder {
    pub fn new(program_id: Pubkey) -> Self {
        Self { program_id }
    }
}

impl DexDecoder for JupiterDecoder {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn dex(&self) -> Dex {
        Dex::JupiterV6
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
        if data.len() < 8 {
            return None;
        }

        let disc = &data[..8];
        let is_shared_route = disc == SHARED_ACCOUNTS_ROUTE_DISC;
        let is_exact_out = disc == SHARED_ACCOUNTS_EXACT_OUT_DISC;

        if !is_shared_route && !is_exact_out {
            return None;
        }

        if data.len() < 11 {
            return None;
        }

        let source_mint_idx = instruction_accounts.get(SOURCE_MINT_INDEX).copied()? as usize;
        let dest_mint_idx = instruction_accounts.get(DEST_MINT_INDEX).copied()? as usize;

        let source_mint = accounts.get(source_mint_idx).copied()?;
        let dest_mint = accounts.get(dest_mint_idx).copied()?;

        // Route plan is variable length, amounts are packed at the tail
        let route_plan_offset = 9;
        if data.len() < route_plan_offset + 4 {
            return None;
        }

        let _route_plan_len = u32::from_le_bytes(
            data[route_plan_offset..route_plan_offset + 4].try_into().ok()?,
        ) as usize;

        // Tail: in_amount(8) + quoted_out(8) + slippage(2) + platform_fee(1) = 19 bytes
        let (in_amount, out_amount, slippage_bps) = if data.len() >= 19 {
            let tail = &data[data.len() - 19..];
            let in_amount = u64::from_le_bytes(tail[0..8].try_into().ok()?);
            let out_amount = u64::from_le_bytes(tail[8..16].try_into().ok()?);
            let slippage = u16::from_le_bytes(tail[16..18].try_into().ok()?);
            (in_amount, Some(out_amount), Some(slippage as u32))
        } else {
            (0u64, None, None)
        };

        let sol_mint = solana_sdk::system_program::ID;
        let wsol_mint: Pubkey = "So11111111111111111111111111111111111111112"
            .parse()
            .unwrap();

        let direction = if source_mint == sol_mint || source_mint == wsol_mint {
            Direction::Buy
        } else {
            Direction::Sell
        };

        tracing::info!(
            dex = "jupiter_v6",
            direction = %direction,
            source_mint = %source_mint,
            dest_mint = %dest_mint,
            in_amount,
            wallet = %wallet,
            signature,
            "Decoded Jupiter v6 swap"
        );

        Some(TradeIntent {
            wallet: *wallet,
            wallet_label: wallet_label.map(|s| s.to_string()),
            target_tx_signature: signature.to_string(),
            slot,
            input_mint: source_mint,
            output_mint: dest_mint,
            input_amount: in_amount,
            output_amount: out_amount,
            slippage_bps,
            direction,
            dex: Dex::JupiterV6,
            detected_at: Utc::now(),
        })
    }
}
