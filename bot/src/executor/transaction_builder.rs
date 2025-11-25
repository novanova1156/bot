// bot/src/executor/transaction_builder.rs
use anchor_lang::prelude::*;
use anchor_spl::{associated_token, token};
use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    sysvar,
};
use solana_sdk::pubkey;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::{
    config::BotConfig,
    dex_structs::{AmmInfo, CpmmPoolInfo, ClmmPoolInfo},
    types::{ArbitrageOpportunity, DexProtocol, SwapLeg},
};

// ============================================================================
// DEX PROGRAM IDS (DUPLICATE FROM SC FOR CLIENT-SIDE ACCOUNT ASSEMBLY)
// ============================================================================
// Убедитесь, что эти ID совпадают с теми, что в lib.rs!
pub const RAYDIUM_AMM_V4: Pubkey = pubkey!("DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav");
pub const RAYDIUM_CPMM: Pubkey = pubkey!("DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb");
pub const RAYDIUM_CLMM: Pubkey = pubkey!("DRayAUgENGQBKVaX8owNhgzkEDyoHTGVEGHVJT1E9pfH");

// SPL Program IDs для CLMM
pub const SPL_TOKEN_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const SPL_TOKEN_2022_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const SPL_MEMO_ID: Pubkey = pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
// ============================================================================

pub struct TransactionBuilder {
    rpc_client: Arc<RpcClient>,
    keypair:    Arc<Keypair>,
    config:     Arc<BotConfig>,
    program_id: Pubkey,
}

/* ---------------- сериализуемые структуры ---------------- */
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
struct ProgramSwapLeg {
    protocol:           u8,
    pool_id:            Pubkey,
    input_mint:         Pubkey,
    output_mint:        Pubkey,
    amount_in:          u64,
    minimum_amount_out: u64,
    accounts_len:       u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
struct ExecuteArbitrageParams {
    swap_legs:           Vec<ProgramSwapLeg>,
    min_profit_lamports: u64,
}

/* ---------------- impl ---------------- */
impl TransactionBuilder {
    pub fn new(
        rpc_client: Arc<RpcClient>,
        keypair: Arc<Keypair>,
        config:  Arc<BotConfig>,
    ) -> Result<Self> {
        Ok(Self {
            program_id: config.trading.executor_program_id.to_pubkey()?,
            rpc_client,
            keypair,
            config,
        })
    }

    /* ---------- публичный API ---------- */
    pub async fn build_arbitrage_transaction(
        &self,
        opp: &ArbitrageOpportunity,
    ) -> Result<Transaction> {
        info!("🔨 Строим транзакцию: {} свопов", opp.legs.len());

        /* ---------- mock-режим для devnet-fallback ---------- */
        let is_test_environment = self.config.rpc.url.contains("devnet") // Используем "devnet" в нижнем регистре
            && opp
            .legs
            .iter()
            .any(|leg| self.rpc_client.get_account(&leg.pool_id).is_err());

        if is_test_environment {
            info!("🧪 ТЕСТОВАЯ СРЕДА: возвращаем mock-транзакцию");

            let mock_tx = Transaction::new_signed_with_payer(
                &[ComputeBudgetInstruction::set_compute_unit_limit(
                    self.config.trading.compute_unit_limit,
                )],
                Some(&self.keypair.pubkey()),
                &[self.keypair.as_ref()],
                self.latest_blockhash()?,
            );

            warn!("⚠️  Пулы фиктивные – реальный RPC не выполняется");
            return Ok(mock_tx);
        }
        /* ----------------------------------------------------- */

        self.validate_pools_exist(opp).await?;

        /* ----- compute budget ----- */
        let mut instructions = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(
                self.config.trading.compute_unit_limit,
            ),
            ComputeBudgetInstruction::set_compute_unit_price(
                self.config.trading.priority_fee_micro_lamports,
            ),
        ];

        /* ----- формируем legs ----- */
        let mut rem_accs = Vec::<AccountMeta>::new();
        let mut prog_legs = Vec::<ProgramSwapLeg>::new();

        for (idx, leg) in opp.legs.iter().enumerate() {
            debug!("⚙️  leg #{} {:?}", idx + 1, leg.protocol);

            let (accs, pl) = self.accounts_for_leg(leg).await?;
            rem_accs.extend(accs);
            prog_legs.push(pl);
        }

        instructions.push(self.make_execute_ix(prog_legs, opp.net_profit, rem_accs)?);

        /* ----- финальный tx ----- */
        let mut tx = Transaction::new_with_payer(&instructions, Some(&self.keypair.pubkey()));
        tx.sign(&[self.keypair.as_ref()], self.latest_blockhash()?);

        Ok(tx)
    }

    /* ---------- helpers ---------- */
    fn latest_blockhash(&self) -> Result<Hash> {
        Ok(self.rpc_client.get_latest_blockhash()?)
    }

    async fn validate_pools_exist(&self, opp: &ArbitrageOpportunity) -> Result<()> {
        for (i, leg) in opp.legs.iter().enumerate() {
            let acc = self
                .rpc_client
                .get_account(&leg.pool_id)
                .with_context(|| format!("RPC get_account {}", leg.pool_id))?;
            if acc.data.is_empty() {
                anyhow::bail!("Пул {} (leg #{}) пустой", leg.pool_id, i + 1);
            }
        }
        Ok(())
    }

    /* ---------- accounts per leg ---------- */
    fn dex_program_id_for_protocol(&self, protocol: DexProtocol) -> Pubkey {
        match protocol {
            DexProtocol::RaydiumAmmV4 => RAYDIUM_AMM_V4,
            DexProtocol::RaydiumCpmm => RAYDIUM_CPMM,
            DexProtocol::RaydiumClmm => RAYDIUM_CLMM,
            // Добавьте другие DEX по мере необходимости
            _ => panic!("Неизвестный протокол DEX"),
        }
    }

    async fn accounts_for_leg(
        &self,
        leg: &SwapLeg,
    ) -> Result<(Vec<AccountMeta>, ProgramSwapLeg)> {
        match leg.protocol {
            DexProtocol::RaydiumAmmV4 => self.raydium_amm_v4_accounts(leg).await,
            DexProtocol::RaydiumCpmm => self.raydium_cpmm_accounts(leg).await,
            DexProtocol::RaydiumClmm => self.get_raydium_clmm_accounts(leg).await,
            _ => unimplemented!("DEX {:?} не реализован", leg.protocol),
        }
    }

    async fn raydium_amm_v4_accounts(
        &self,
        leg: &SwapLeg,
    ) -> Result<(Vec<AccountMeta>, ProgramSwapLeg)> {
        let data = self.rpc_client.get_account(&leg.pool_id)?.data;
        let amm  = AmmInfo::try_from_slice(&data).context("decode AmmInfo")?;

        // ID программы DEX *не* включается в список аккаунтов для CPI
        let dex_program_id = self.dex_program_id_for_protocol(leg.protocol);

        let user_src = associated_token::get_associated_token_address(&self.keypair.pubkey(), &leg.input_mint);
        let user_dst = associated_token::get_associated_token_address(&self.keypair.pubkey(), &leg.output_mint);

        // Raydium AMM V4 требует 18 аккаунтов.
        let accts = vec![
            // ИСПРАВЛЕНО: ВОЗВРАЩАЕМ Program ID. Это 1-й аккаунт для SC (для invoke).
            AccountMeta::new_readonly(dex_program_id, false),

            // 8 стандартных аккаунтов, которые мы знаем (18 всего)
            AccountMeta::new(leg.pool_id, false),
            AccountMeta::new_readonly(amm.market_id, false),
            AccountMeta::new(amm.base_vault, false),
            AccountMeta::new(amm.quote_vault, false),
            AccountMeta::new(user_src, false),
            AccountMeta::new(user_dst, false),
            AccountMeta::new_readonly(self.keypair.pubkey(), true),
            AccountMeta::new_readonly(token::ID, false),
            // ... здесь не хватает 10 аккаунтов для V4, но это отдельная проблема
        ];

        let pl = ProgramSwapLeg {
            protocol:           leg.protocol as u8,
            pool_id:            leg.pool_id,
            input_mint:         leg.input_mint,
            output_mint:        leg.output_mint,
            amount_in:          leg.amount_in,
            minimum_amount_out: leg.minimum_amount_out,
            accounts_len:       accts.len() as u8, // 9 аккаунтов (DEX ID + 8)
        };

        Ok((accts, pl))
    }

    async fn raydium_cpmm_accounts(
        &self,
        leg: &SwapLeg,
    ) -> Result<(Vec<AccountMeta>, ProgramSwapLeg)> {

        let data = self.rpc_client.get_account(&leg.pool_id)?.data;

        let pool_info = CpmmPoolInfo::try_from_slice(&data)
            .with_context(|| format!("Не удалось декодировать CpmmPoolInfo для пула {}", leg.pool_id))?;

        let (authority, vault_a, vault_b, mint_a) =
            (pool_info.authority, pool_info.vault_a, pool_info.vault_b, pool_info.mint_a);


        let (token_vault_in, token_vault_out) = if leg.input_mint == mint_a {
            (vault_a, vault_b)
        } else {
            (vault_b, vault_a)
        };

        let dex_program_id = self.dex_program_id_for_protocol(leg.protocol);
        let user_src = associated_token::get_associated_token_address(&self.keypair.pubkey(), &leg.input_mint);
        let user_dst = associated_token::get_associated_token_address(&self.keypair.pubkey(), &leg.output_mint);

        // 10 аккаунтов для CPMM (DEX ID + 9)
        let accts = vec![
            // ИСПРАВЛЕНО: ВОЗВРАЩАЕМ Program ID. Это 1-й аккаунт для SC (для invoke).
            AccountMeta::new_readonly(dex_program_id, false),

            // 9 стандартных Raydium CPI аккаунтов (начиная со 2-го аккаунта в списке)
            AccountMeta::new(leg.pool_id, false),                    // 1. Пул/Стейт (Mut)
            AccountMeta::new_readonly(authority, false),             // 2. Authority пула (Readonly)
            AccountMeta::new(token_vault_in, false),                 // 3. Vault IN (Mut)
            AccountMeta::new(token_vault_out, false),                // 4. Vault OUT (Mut)
            AccountMeta::new(user_src, false),                       // 5. ATA From (Mut)
            AccountMeta::new(user_dst, false),                       // 6. ATA To (Mut)
            AccountMeta::new_readonly(self.keypair.pubkey(), true),  // 7. Signer/Инициатор (Readonly/Signer)
            AccountMeta::new_readonly(token::ID, false),             // 8. Token Program (Readonly)
            AccountMeta::new_readonly(sysvar::clock::ID, false),     // 9. Sysvar Clock (Readonly)
        ];

        let accounts_len = accts.len() as u8;

        let pl = ProgramSwapLeg {
            protocol:           leg.protocol as u8,
            pool_id:            leg.pool_id,
            input_mint:         leg.input_mint,
            output_mint:        leg.output_mint,
            amount_in:          leg.amount_in,
            minimum_amount_out: leg.minimum_amount_out,
            accounts_len:       accounts_len, // Теперь 10 для CPMM (1+9)
        };

        Ok((accts, pl))
    }

    async fn get_raydium_clmm_accounts(
        &self,
        leg: &SwapLeg,
    ) -> Result<(Vec<AccountMeta>, ProgramSwapLeg)> {
        debug!("📊 Получение аккаунтов для Raydium CLMM пула: {}", leg.pool_id);

        let pool_account = self.rpc_client.get_account(&leg.pool_id)?;
        let pool_data = &pool_account.data[8..]; // Пропускаем Anchor discriminator

        let amm_config = Pubkey::new_from_array(pool_data[1..33].try_into().map_err(|_| {
            anyhow::anyhow!("Не удалось извлечь amm_config из pool data")
        })?);
        let authority = Pubkey::new_from_array(pool_data[33..65].try_into().map_err(|_| {
            anyhow::anyhow!("Не удалось извлечь authority из pool data")
        })?);
        let token_mint_0 = Pubkey::new_from_array(pool_data[65..97].try_into().map_err(|_| {
            anyhow::anyhow!("Не удалось извлечь token_mint_0 из pool data")
        })?);
        let token_mint_1 = Pubkey::new_from_array(pool_data[97..129].try_into().map_err(|_| {
            anyhow::anyhow!("Не удалось извлечь token_mint_1 из pool data")
        })?);
        let token_vault_0 = Pubkey::new_from_array(pool_data[129..161].try_into().map_err(|_| {
            anyhow::anyhow!("Не удалось извлечь token_vault_0 из pool data")
        })?);
        let token_vault_1 = Pubkey::new_from_array(pool_data[161..193].try_into().map_err(|_| {
            anyhow::anyhow!("Не удалось извлечь token_vault_1 из pool data")
        })?);
        let observation_key = Pubkey::new_from_array(pool_data[193..225].try_into().map_err(|_| {
            anyhow::anyhow!("Не удалось извлечь observation_key из pool data")
        })?);

        let (input_vault, output_vault) = if leg.input_mint == token_mint_0 {
            (token_vault_0, token_vault_1)
        } else {
            (token_vault_1, token_vault_0)
        };

        let user_input_ata = associated_token::get_associated_token_address(
            &self.keypair.pubkey(),
            &leg.input_mint
        );
        let user_output_ata = associated_token::get_associated_token_address(
            &self.keypair.pubkey(),
            &leg.output_mint
        );

        let dex_program_id = self.dex_program_id_for_protocol(leg.protocol);


        // 13 фиксированных аккаунтов для CLMM swap_v2 (согласно официальной структуре)
        let accounts = vec![
            // ИСПРАВЛЕНО: ВОЗВРАЩАЕМ Program ID. Это 1-й аккаунт для SC (для invoke).
            AccountMeta::new_readonly(dex_program_id, false),

            // 0. payer (signer) - Это наш Payer (Keypair)
            AccountMeta::new(self.keypair.pubkey(), true),
            // 1. amm_config
            AccountMeta::new_readonly(amm_config, false),
            // 2. pool_state
            AccountMeta::new(leg.pool_id, false),
            // 3. input_token_account (ATA пользователя)
            AccountMeta::new(user_input_ata, false),
            // 4. output_token_account (ATA пользователя)
            AccountMeta::new(user_output_ata, false),
            // 5. input_vault
            AccountMeta::new(input_vault, false),
            // 6. output_vault
            AccountMeta::new(output_vault, false),
            // 7. observation_state
            AccountMeta::new(observation_key, false),
            // 8. token_program (Используем константу)
            AccountMeta::new_readonly(SPL_TOKEN_ID, false),
            // 9. token_program2022 (Используем константу)
            AccountMeta::new_readonly(SPL_TOKEN_2022_ID, false),
            // 10. memo_program (Используем константу)
            AccountMeta::new_readonly(SPL_MEMO_ID, false),
            // 11. input_vault_mint
            AccountMeta::new_readonly(leg.input_mint, false),
            // 12. output_vault_mint
            AccountMeta::new_readonly(leg.output_mint, false),

            // Remaining accounts: tick arrays (TODO: добавить динамически на основе swap размера)
            // Для простоты пока не добавляем; в продакшене нужно вычислить и добавить 1-3 tick array PDA
        ];

        debug!("   ✅ Подготовлено {} аккаунтов для Raydium CLMM (14 fixed + tick arrays TBD)", accounts.len());

        let program_leg = ProgramSwapLeg {
            protocol: leg.protocol as u8,
            pool_id: leg.pool_id,
            input_mint: leg.input_mint,
            output_mint: leg.output_mint,
            amount_in: leg.amount_in,
            minimum_amount_out: leg.minimum_amount_out,
            accounts_len: accounts.len() as u8, // Теперь 14 (1 + 13)
        };

        Ok((accounts, program_leg))
    }

    /* ---------- execute-ix ---------- */
    fn make_execute_ix(
        &self,
        legs: Vec<ProgramSwapLeg>,
        min_profit: u64,
        mut rem: Vec<AccountMeta>,
    ) -> Result<Instruction> {
        let first_mint = legs.first().context("legs empty")?.input_mint;
        let user_ata =
            associated_token::get_associated_token_address(&self.keypair.pubkey(), &first_mint);

        let mut accs = vec![
            AccountMeta::new(self.keypair.pubkey(), true),
            AccountMeta::new(user_ata, false),
            AccountMeta::new_readonly(first_mint, false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ];
        accs.append(&mut rem);

        Ok(Instruction {
            program_id: self.program_id,
            accounts:   accs,
            data:       self.build_ix_data(legs, min_profit)?,
        })
    }

    fn build_ix_data(&self, legs: Vec<ProgramSwapLeg>, min_profit: u64) -> Result<Vec<u8>> {
        const DISC: [u8; 8] = [0x3f, 0x39, 0x4c, 0x8f, 0x29, 0x34, 0x70, 0xd0];
        let params = ExecuteArbitrageParams { swap_legs: legs, min_profit_lamports: min_profit };
        let mut data = DISC.to_vec();
        data.extend_from_slice(&params.try_to_vec()?);
        Ok(data)
    }
}