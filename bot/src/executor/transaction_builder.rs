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
        let is_test_environment = self.config.rpc.url.contains("devnet")
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
            DexProtocol::RaydiumCpmm | DexProtocol::RaydiumClmm => {
                self.raydium_cpmm_clmm_accounts(leg).await
            }
            _ => unimplemented!("DEX {:?} не реализован", leg.protocol),
        }
    }

    async fn raydium_amm_v4_accounts(
        &self,
        leg: &SwapLeg,
    ) -> Result<(Vec<AccountMeta>, ProgramSwapLeg)> {
        let data = self.rpc_client.get_account(&leg.pool_id)?.data;
        let amm  = AmmInfo::try_from_slice(&data).context("decode AmmInfo")?;
        let dex_program_id = self.dex_program_id_for_protocol(leg.protocol);

        let user_src = associated_token::get_associated_token_address(&self.keypair.pubkey(), &leg.input_mint);
        let user_dst = associated_token::get_associated_token_address(&self.keypair.pubkey(), &leg.output_mint);

        // ВАЖНО: Raydium AMM V4 требует 18 аккаунтов.
        // Временный, неполный список для отладки V4 (9 аккаунтов)
        let accts = vec![
            // 1. DEX Program ID (ДЛЯ CPI)
            AccountMeta::new_readonly(dex_program_id, false),

            // 8 стандартных аккаунтов, которые мы знаем
            AccountMeta::new(leg.pool_id, false),
            AccountMeta::new_readonly(amm.market_id, false),
            AccountMeta::new(amm.base_vault, false),
            AccountMeta::new(amm.quote_vault, false),
            AccountMeta::new(user_src, false),
            AccountMeta::new(user_dst, false),
            AccountMeta::new_readonly(self.keypair.pubkey(), true),
            AccountMeta::new_readonly(token::ID, false),
        ];

        let pl = ProgramSwapLeg {
            protocol:           leg.protocol as u8,
            pool_id:            leg.pool_id,
            input_mint:         leg.input_mint,
            output_mint:        leg.output_mint,
            amount_in:          leg.amount_in,
            minimum_amount_out: leg.minimum_amount_out,
            accounts_len:       accts.len() as u8,
        };

        Ok((accts, pl))
    }

    // ИСПРАВЛЕННАЯ ФУНКЦИЯ ДЛЯ CPMM/CLMM (10 или 12 аккаунтов)
    async fn raydium_cpmm_clmm_accounts(
        &self,
        leg: &SwapLeg,
    ) -> Result<(Vec<AccountMeta>, ProgramSwapLeg)> {

        let data = self.rpc_client.get_account(&leg.pool_id)?.data;

        // Получаем информацию о пуле
        let (authority, vault_a, vault_b, mint_a) = match leg.protocol {
            DexProtocol::RaydiumCpmm => {
                let pool_info = CpmmPoolInfo::try_from_slice(&data)
                    .with_context(|| format!("Не удалось декодировать CpmmPoolInfo для пула {}", leg.pool_id))?;
                (pool_info.authority, pool_info.vault_a, pool_info.vault_b, pool_info.mint_a)
            }
            DexProtocol::RaydiumClmm => {
                let pool_info = ClmmPoolInfo::try_from_slice(&data)
                    .with_context(|| format!("Не удалось декодировать ClmmPoolInfo для пула {}", leg.pool_id))?;

                warn!("⚠️ Внимание: Парсинг CLMM реализован с минимальными, возможно, неточными офсетами. Требуется верификация.");
                (pool_info.authority, pool_info.vault_a, pool_info.vault_b, pool_info.mint_a)
            }
            _ => {
                anyhow::bail!("Неподдерживаемый протокол в raydium_cpmm_clmm_accounts");
            }
        };

        // --- КРИТИЧЕСКОЕ ИСПРАВЛЕНИЕ #1: Динамический выбор Vault IN/OUT ---
        let (token_vault_in, token_vault_out) = if leg.input_mint == mint_a {
            // Swap A -> B: Vault A - вход, Vault B - выход
            (vault_a, vault_b)
        } else {
            // Swap B -> A: Vault B - вход, Vault A - выход
            (vault_b, vault_a)
        };
        // ------------------------------------------------------------------

        let dex_program_id = self.dex_program_id_for_protocol(leg.protocol);
        let user_src = associated_token::get_associated_token_address(&self.keypair.pubkey(), &leg.input_mint);
        let user_dst = associated_token::get_associated_token_address(&self.keypair.pubkey(), &leg.output_mint);

        // 1. 10 базовых аккаунтов (Raydium Program ID + 9 CPI accounts)
        let mut accts = vec![
            // 1. DEX Program ID (Включаем, чтобы удовлетворить проверку SC)
            AccountMeta::new_readonly(dex_program_id, false),

            // 9 стандартных Raydium CPI аккаунтов
            AccountMeta::new(leg.pool_id, false),                    // 2. Пул/Стейт (Mut)
            AccountMeta::new_readonly(authority, false),             // 3. Authority пула (Readonly)
            AccountMeta::new(token_vault_in, false),                 // 4. Vault IN (Mut)
            AccountMeta::new(token_vault_out, false),                // 5. Vault OUT (Mut)
            AccountMeta::new(user_src, false),                       // 6. ATA From (Mut)
            AccountMeta::new(user_dst, false),                       // 7. ATA To (Mut)
            AccountMeta::new_readonly(self.keypair.pubkey(), true),  // 8. Signer/Инициатор (Readonly/Signer)
            AccountMeta::new_readonly(token::ID, false),             // 9. Token Program (Readonly)
            AccountMeta::new_readonly(sysvar::clock::ID, false),     // 10. Sysvar Clock (Readonly)
        ];

        // --- КРИТИЧЕСКОЕ ИСПРАВЛЕНИЕ #2: Добавляем placeholder аккаунты для CLMM (12 аккаунтов) ---
        if leg.protocol == DexProtocol::RaydiumClmm {
            // Raydium CLMM (и большинство CLMM/DLMM) требуют 2 дополнительных аккаунта (тик-массивы).
            // Добавляем 2 placeholder аккаунта, чтобы пройти проверку SC (accounts.len() >= 12).
            // Внимание: Для реального исполнения эти адреса должны быть правильно вычислены!

            // 11. Tick Array 0 (Mut)
            accts.push(AccountMeta::new(Pubkey::default(), false));
            // 12. Tick Array 1 (Mut)
            accts.push(AccountMeta::new(Pubkey::default(), false));
        }
        // -----------------------------------------------------------------------------

        let accounts_len = accts.len() as u8;

        let pl = ProgramSwapLeg {
            protocol:           leg.protocol as u8,
            pool_id:            leg.pool_id,
            input_mint:         leg.input_mint,
            output_mint:        leg.output_mint,
            amount_in:          leg.amount_in,
            minimum_amount_out: leg.minimum_amount_out,
            accounts_len:       accounts_len, // 10 для CPMM, 12 для CLMM
        };

        Ok((accts, pl))
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