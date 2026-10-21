use crate::config::ApexConfig;
use orbitflare_apex::rpc::RpcClient as ApexRpcClient;
use orbitflare_apex::{tip, tip_instruction, ApexSenderClient, ClientOptions, Region};
use solana_sdk::instruction::CompiledInstruction;
use solana_sdk::message::{MessageHeader, VersionedMessage};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::VersionedTransaction;

pub struct ApexClient {
    quic: ApexSenderClient,
    http: ApexRpcClient,
    tip_accounts: Vec<orbitflare_apex::Pubkey>,
    tip_lamports: u64,
    mev_protect: bool,
}

impl ApexClient {
    pub async fn connect(config: &ApexConfig) -> anyhow::Result<Self> {
        let region = Region::parse(&config.region)
            .ok_or_else(|| anyhow::anyhow!("Unknown Apex region: {}", config.region))?;
        let http = ApexRpcClient::new(region, &config.api_key);
        let tip_accounts = http.get_tip_accounts().await?;
        if tip_accounts.is_empty() {
            anyhow::bail!("Apex returned no tip accounts");
        }
        let quic = ApexSenderClient::connect_with_options(
            ClientOptions {
                endpoint: Some(region.quic_endpoint()),
                mev_protect: config.mev_protect,
                ..Default::default()
            },
            &config.api_key,
        )
        .await?;

        tracing::info!(
            region = region.code(),
            tip_lamports = config.tip_lamports,
            "Connected to OrbitFlare Apex"
        );

        Ok(Self {
            quic,
            http,
            tip_accounts,
            tip_lamports: config.tip_lamports,
            mev_protect: config.mev_protect,
        })
    }

    pub async fn send_transaction(
        &self,
        tx_base64: &str,
        keypair: &Keypair,
    ) -> anyhow::Result<String> {
        let wire = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, tx_base64)?;
        let tx: VersionedTransaction = bincode::deserialize(&wire)?;
        let mut message = tx.message;
        self.append_tip(&mut message, &keypair.pubkey())?;

        let tx = VersionedTransaction::try_new(message, &[keypair])?;
        let signature = tx.signatures[0].to_string();
        let wire = bincode::serialize(&tx)?;

        if let Err(e) = self.quic.send_transaction_bytes(wire.clone().into()).await {
            tracing::warn!(error = %e, "Apex QUIC send failed, retrying over HTTP");
            self.http
                .send_transaction(&wire, self.mev_protect, None)
                .await?;
        }

        tracing::info!(
            signature = %signature,
            tip_lamports = self.tip_lamports,
            "Transaction submitted to Apex"
        );

        Ok(signature)
    }

    fn append_tip(&self, message: &mut VersionedMessage, payer: &Pubkey) -> anyhow::Result<()> {
        let tip_account = tip::pick_tip_account(&self.tip_accounts)
            .ok_or_else(|| anyhow::anyhow!("No Apex tip accounts"))?;
        let ix = tip_instruction(
            &orbitflare_apex::Pubkey::new_from_array(payer.to_bytes()),
            &tip_account,
            self.tip_lamports,
        );
        let to_pubkey = |key: &orbitflare_apex::Pubkey| Pubkey::new_from_array(key.to_bytes());

        let (header, keys, instructions) = match message {
            VersionedMessage::Legacy(m) => {
                (&mut m.header, &mut m.account_keys, &mut m.instructions)
            }
            VersionedMessage::V0(m) => (&mut m.header, &mut m.account_keys, &mut m.instructions),
        };

        let mut accounts = Vec::with_capacity(ix.accounts.len());
        for meta in &ix.accounts {
            accounts.push(static_key_index(
                header,
                keys,
                instructions,
                to_pubkey(&meta.pubkey),
                meta.is_writable,
            )?);
        }
        let program_id_index =
            static_key_index(header, keys, instructions, to_pubkey(&ix.program_id), false)?;

        instructions.push(CompiledInstruction {
            program_id_index,
            accounts,
            data: ix.data,
        });
        Ok(())
    }
}

fn static_key_index(
    header: &mut MessageHeader,
    keys: &mut Vec<Pubkey>,
    instructions: &mut [CompiledInstruction],
    key: Pubkey,
    writable: bool,
) -> anyhow::Result<u8> {
    if let Some(index) = keys.iter().position(|k| *k == key) {
        let index = index as u8;
        let signers = header.num_required_signatures;
        let is_writable = if index < signers {
            index < signers - header.num_readonly_signed_accounts
        } else {
            (index as usize) < keys.len() - header.num_readonly_unsigned_accounts as usize
        };
        if writable && !is_writable {
            anyhow::bail!("{} is read-only in the swap transaction", key);
        }
        return Ok(index);
    }

    let index = if writable {
        keys.len() - header.num_readonly_unsigned_accounts as usize
    } else {
        header.num_readonly_unsigned_accounts += 1;
        keys.len()
    };
    let index = u8::try_from(index)?;

    for ix in instructions.iter_mut() {
        if ix.program_id_index >= index {
            ix.program_id_index += 1;
        }
        for account in ix.accounts.iter_mut() {
            if *account >= index {
                *account += 1;
            }
        }
    }
    keys.insert(index as usize, key);
    Ok(index)
}
