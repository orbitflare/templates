use crate::decoder::DexDecoder;
use crate::types::{Dex, Direction, TradeIntent};
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;

const AMM_SWAP_BASE_IN: u8 = 9;
const AMM_SWAP_BASE_OUT: u8 = 11;

// Raydium AMM account layout for swap:
// 0: token program
// 1: amm id
// 2: amm authority
// 3: amm open orders
// 4: amm target orders (or pool coin token account for newer versions)
// 5: pool coin token account
// 6: pool pc token account
// 7: serum program
// 8: serum market
// 9: serum bids
// 10: serum asks
// 11: serum event queue
// 12: serum coin vault
// 13: serum pc vault
// 14: serum vault signer
// 15: user source token account
// 16: user dest token account
// 17: user owner (signer)

const AMM_POOL_COIN_INDEX: usize = 5;
const AMM_POOL_PC_INDEX: usize = 6;
const AMM_USER_SOURCE_INDEX: usize = 15;
const AMM_USER_DEST_INDEX: usize = 16;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

pub struct RaydiumAmmDecoder {
    program_id: Pubkey,
    mint_cache: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<Pubkey, Pubkey>>>,
}

impl RaydiumAmmDecoder {
    pub fn new(program_id: Pubkey) -> Self {
        Self {
            program_id,
            mint_cache: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    pub fn mint_cache(
        &self,
    ) -> std::sync::Arc<std::sync::RwLock<std::collections::HashMap<Pubkey, Pubkey>>> {
        self.mint_cache.clone()
    }
}

impl DexDecoder for RaydiumAmmDecoder {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn dex(&self) -> Dex {
        Dex::RaydiumAmm
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
        if data.is_empty() {
            return None;
        }

        let instruction_type = data[0];
        let is_swap = instruction_type == AMM_SWAP_BASE_IN || instruction_type == AMM_SWAP_BASE_OUT;

        if !is_swap {
            return None;
        }

        if data.len() < 17 {
            return None;
        }

        let amount_a = u64::from_le_bytes(data[1..9].try_into().ok()?);
        let amount_b = u64::from_le_bytes(data[9..17].try_into().ok()?);

        let (input_amount, output_amount) = if instruction_type == AMM_SWAP_BASE_IN {
            (amount_a, Some(amount_b))
        } else {
            (amount_b, Some(amount_a))
        };

        let pool_coin_idx = instruction_accounts.get(AMM_POOL_COIN_INDEX).copied()? as usize;
        let pool_pc_idx = instruction_accounts.get(AMM_POOL_PC_INDEX).copied()? as usize;
        let user_source_idx = instruction_accounts.get(AMM_USER_SOURCE_INDEX).copied()? as usize;
        let user_dest_idx = instruction_accounts.get(AMM_USER_DEST_INDEX).copied()? as usize;

        let pool_coin = accounts.get(pool_coin_idx).copied()?;
        let pool_pc = accounts.get(pool_pc_idx).copied()?;
        let user_source = accounts.get(user_source_idx).copied()?;
        let user_dest = accounts.get(user_dest_idx).copied()?;

        let wsol: Pubkey = WSOL_MINT.parse().unwrap();
        let sol_mint = solana_sdk::system_program::ID;

        let cache = self.mint_cache.read().unwrap_or_else(|e| e.into_inner());
        let source_mint = cache.get(&user_source).copied();
        let dest_mint = cache.get(&user_dest).copied();
        drop(cache);

        let swapping_coin_to_pc = user_source == pool_coin
            || cache_matches_pool(&source_mint, &pool_coin);

        let (input_mint, output_mint, direction) = if let (Some(src_m), Some(dst_m)) =
            (source_mint, dest_mint)
        {
            let dir = if src_m == sol_mint || src_m == wsol {
                Direction::Buy
            } else if dst_m == sol_mint || dst_m == wsol || swapping_coin_to_pc {
                Direction::Sell
            } else {
                Direction::Buy
            };
            (src_m, dst_m, dir)
        } else {
            let dir = if swapping_coin_to_pc {
                Direction::Sell
            } else {
                Direction::Buy
            };

            if swapping_coin_to_pc {
                (pool_coin, pool_pc, dir)
            } else {
                (pool_pc, pool_coin, dir)
            }
        };

        tracing::info!(
            dex = "raydium_amm",
            direction = %direction,
            input_mint = %input_mint,
            output_mint = %output_mint,
            pool_coin = %pool_coin,
            pool_pc = %pool_pc,
            input_amount,
            wallet = %wallet,
            signature,
            cached_mints = source_mint.is_some() && dest_mint.is_some(),
            "Decoded Raydium AMM swap"
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
            dex: Dex::RaydiumAmm,
            detected_at: Utc::now(),
        })
    }
}

fn cache_matches_pool(_cached_mint: &Option<Pubkey>, _pool_account: &Pubkey) -> bool {
    false
}

const CPMM_SWAP_BASE_INPUT_DISC: [u8; 8] = [143, 190, 90, 218, 196, 30, 51, 222];
const CPMM_SWAP_BASE_OUTPUT_DISC: [u8; 8] = [55, 217, 98, 86, 163, 74, 180, 173];

// CPMM account layout for swap:
// 0: payer (signer)
// 1: authority
// 2: amm config
// 3: pool state
// 4: input token account
// 5: output token account
// 6: input vault
// 7: output vault
// 8: input token program
// 9: output token program
// 10: input token mint
// 11: output token mint
// 12: observation state

const CPMM_INPUT_MINT_INDEX: usize = 10;
const CPMM_OUTPUT_MINT_INDEX: usize = 11;

pub struct RaydiumCpmmDecoder {
    program_id: Pubkey,
}

impl RaydiumCpmmDecoder {
    pub fn new(program_id: Pubkey) -> Self {
        Self { program_id }
    }
}

impl DexDecoder for RaydiumCpmmDecoder {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn dex(&self) -> Dex {
        Dex::RaydiumCpmm
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
        let is_base_input = disc == CPMM_SWAP_BASE_INPUT_DISC;
        let is_base_output = disc == CPMM_SWAP_BASE_OUTPUT_DISC;

        if !is_base_input && !is_base_output {
            return None;
        }

        let amount_a = u64::from_le_bytes(data[8..16].try_into().ok()?);
        let amount_b = u64::from_le_bytes(data[16..24].try_into().ok()?);

        let (input_amount, output_amount) = if is_base_input {
            (amount_a, Some(amount_b))
        } else {
            (amount_b, Some(amount_a))
        };

        let input_mint_idx = instruction_accounts.get(CPMM_INPUT_MINT_INDEX).copied()? as usize;
        let output_mint_idx = instruction_accounts.get(CPMM_OUTPUT_MINT_INDEX).copied()? as usize;

        let input_mint = accounts.get(input_mint_idx).copied()?;
        let output_mint = accounts.get(output_mint_idx).copied()?;

        let sol_mint = solana_sdk::system_program::ID;
        let wsol_mint: Pubkey = "So11111111111111111111111111111111111111112"
            .parse()
            .unwrap();

        let direction = if input_mint == sol_mint || input_mint == wsol_mint {
            Direction::Buy
        } else {
            Direction::Sell
        };

        tracing::info!(
            dex = "raydium_cpmm",
            direction = %direction,
            input_mint = %input_mint,
            output_mint = %output_mint,
            input_amount,
            wallet = %wallet,
            signature,
            "Decoded Raydium CPMM swap"
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
            dex: Dex::RaydiumCpmm,
            detected_at: Utc::now(),
        })
    }
}
