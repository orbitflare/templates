pub mod jupiter;
pub mod raydium;
pub mod pumpfun;

use crate::config::AppConfig;
use crate::types::{Dex, TradeIntent};
use jetstream_protos::jetstream::SubscribeUpdateTransactionInfo;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

pub trait DexDecoder: Send + Sync {
    fn program_id(&self) -> Pubkey;
    fn dex(&self) -> Dex;
    #[allow(clippy::too_many_arguments)]
    fn decode(
        &self,
        accounts: &[Pubkey],
        instruction_accounts: &[u8],
        data: &[u8],
        wallet: &Pubkey,
        wallet_label: Option<&str>,
        signature: &str,
        slot: u64,
    ) -> Option<TradeIntent>;
}

pub struct DecoderPipeline {
    decoders: Vec<Box<dyn DexDecoder>>,
    config: Arc<AppConfig>,
}

impl DecoderPipeline {
    pub fn from_config(config: Arc<AppConfig>) -> Self {
        let mut decoders: Vec<Box<dyn DexDecoder>> = Vec::new();

        if config.decoders.jupiter.enabled {
            let pid = config.decoders.jupiter.program_id.as_str();
            if let Ok(pubkey) = pid.parse::<Pubkey>() {
                decoders.push(Box::new(jupiter::JupiterDecoder::new(pubkey)));
                tracing::info!(program_id = pid, "Jupiter v6 decoder enabled");
            } else {
                tracing::warn!(program_id = pid, "Invalid Jupiter program ID, skipping");
            }
        }

        if config.decoders.raydium_amm.enabled {
            let pid = config.decoders.raydium_amm.program_id.as_str();
            if let Ok(pubkey) = pid.parse::<Pubkey>() {
                decoders.push(Box::new(raydium::RaydiumAmmDecoder::new(pubkey)));
                tracing::info!(program_id = pid, "Raydium AMM decoder enabled");
            } else {
                tracing::warn!(program_id = pid, "Invalid Raydium AMM program ID, skipping");
            }
        }

        if config.decoders.raydium_cpmm.enabled {
            let pid = config.decoders.raydium_cpmm.program_id.as_str();
            if let Ok(pubkey) = pid.parse::<Pubkey>() {
                decoders.push(Box::new(raydium::RaydiumCpmmDecoder::new(pubkey)));
                tracing::info!(program_id = pid, "Raydium CPMM decoder enabled");
            } else {
                tracing::warn!(program_id = pid, "Invalid Raydium CPMM program ID, skipping");
            }
        }

        if config.decoders.pumpfun.enabled {
            let pid = config.decoders.pumpfun.program_id.as_str();
            if let Ok(pubkey) = pid.parse::<Pubkey>() {
                decoders.push(Box::new(pumpfun::PumpFunDecoder::new(pubkey)));
                tracing::info!(program_id = pid, "Pump.fun decoder enabled");
            } else {
                tracing::warn!(program_id = pid, "Invalid Pump.fun program ID, skipping");
            }
        }

        tracing::info!(decoder_count = decoders.len(), "DecoderPipeline initialized");

        Self { decoders, config }
    }

    pub fn decode_transaction(
        &self,
        tx_info: &SubscribeUpdateTransactionInfo,
    ) -> Vec<TradeIntent> {
        let signature = bs58::encode(&tx_info.signature).into_string();

        let accounts: Vec<Pubkey> = tx_info
            .account_keys
            .iter()
            .filter_map(|bytes| {
                if bytes.len() == 32 {
                    let mut array = [0u8; 32];
                    array.copy_from_slice(bytes);
                    Some(Pubkey::new_from_array(array))
                } else {
                    None
                }
            })
            .collect();

        if accounts.is_empty() {
            return vec![];
        }

        let signer = &accounts[0];
        let signer_str = signer.to_string();
        let wallet_label = self
            .config
            .targets
            .iter()
            .find(|t| t.address == signer_str)
            .and_then(|t| t.label.as_deref());

        let mut intents = Vec::new();

        for instruction in &tx_info.instructions {
            let program_idx = instruction.program_id_index as usize;
            let Some(program_id) = accounts.get(program_idx) else {
                continue;
            };

            let ix_account_indices: &[u8] = &instruction.accounts;

            for decoder in &self.decoders {
                if *program_id == decoder.program_id() {
                    if let Some(intent) = decoder.decode(
                        &accounts,
                        ix_account_indices,
                        &instruction.data,
                        signer,
                        wallet_label,
                        &signature,
                        tx_info.slot,
                    ) {
                        intents.push(intent);
                    }
                }
            }
        }

        intents
    }
}
