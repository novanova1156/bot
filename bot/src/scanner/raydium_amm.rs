// bot/src/scanner/raydium_amm.rs
use anyhow::{Result, Context};
use rayon::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::RpcFilterType;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_program_pack::Pack;
use spl_token::state::Account as TokenAccount;
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, warn, debug};

use crate::config::BotConfig;
use crate::dex_structs::AmmInfo;
use crate::types::{DexProtocol, PoolState};
use super::DexScanner;

#[derive(Clone)]
pub struct RaydiumAmmScanner {
    rpc_client: Arc<RpcClient>,
    config: Arc<BotConfig>,
    program_id: Pubkey,
}

impl RaydiumAmmScanner {
    pub fn new(config: Arc<BotConfig>, rpc_client: Arc<RpcClient>) -> Result<Self> {
        let program_id = config.dex.raydium_amm_v4.to_pubkey()
            .context("Некорректный Raydium AMM program ID в конфигурации")?;

        info!("🚀 Инициализация Raydium AMM сканера с program_id: {}", program_id);

        Ok(Self {
            rpc_client,
            config,
            program_id,
        })
    }

    /// СИНХРОННАЯ функция парсинга для совместимости с rayon
    fn parse_pool_account_sync(&self, pool_id: Pubkey, data: &[u8]) -> Result<PoolState> {
        debug!("🔍 Парсинг пула {} (размер данных: {} байт)", pool_id, data.len());

        let amm_info = AmmInfo::try_from_slice(data)
            .context("Ошибка десериализации AmmInfo")?;

        // ВАЛИДАЦИЯ OpenBook ID
        let expected_openbook_id = self.config.dex.openbook_id.to_pubkey()
            .context("Некорректный OpenBook ID в конфигурации")?;

        if amm_info.market_program_id != expected_openbook_id {
            return Err(anyhow::anyhow!(
                "Неверный market_program_id: ожидался {}, получен {}",
                expected_openbook_id,
                amm_info.market_program_id
            ));
        }

        // Создаем PoolState с базовыми данными (резервы будут получены отдельно)
        Ok(PoolState {
            id: pool_id,
            protocol: DexProtocol::RaydiumAmmV4,
            token_a: amm_info.base_mint,
            token_b: amm_info.quote_mint,
            reserve_a: 0, // Будет обновлено в fetch_vault_reserves_batch
            reserve_b: 0,
            fee_bps: (amm_info.fees.swap_fee_numerator * 10000 / amm_info.fees.swap_fee_denominator) as u16,
            last_updated: chrono::Utc::now().timestamp(),
            full_state_data: data.to_vec(),
            decimals_a: 9, // ДОБАВЛЕНО
            decimals_b: 9, // ДОБАВЛЕНО
        })
    }

    /// ПАКЕТНОЕ получение резервов vault'ов
    fn fetch_vault_reserves_batch(&self, pools: &mut [PoolState]) -> Result<()> {
        if pools.is_empty() {
            return Ok(());
        }

        // Собираем все уникальные vault адреса
        let mut vault_keys = Vec::new();
        let mut pool_vault_map = HashMap::new();

        for (pool_idx, pool) in pools.iter().enumerate() {
            // Парсим amm_info для получения vault адресов
            if let Ok(amm_info) = AmmInfo::try_from_slice(&pool.full_state_data) {
                vault_keys.push(amm_info.base_vault);
                vault_keys.push(amm_info.quote_vault);

                pool_vault_map.insert(amm_info.base_vault, (pool_idx, true));  // true = base
                pool_vault_map.insert(amm_info.quote_vault, (pool_idx, false)); // false = quote
            }
        }

        // Убираем дубликаты
        vault_keys.sort();
        vault_keys.dedup();

        info!("📊 Получение резервов для {} vault'ов", vault_keys.len());

        // ПАКЕТНЫЕ запросы по 100 аккаунтов
        let vault_accounts = self.get_multiple_accounts_batch(&vault_keys)?;

        // Обновляем резервы в pools
        for (vault_key, account_opt) in vault_keys.iter().zip(vault_accounts.iter()) {
            if let (Some(account), Some((pool_idx, is_base))) = (account_opt, pool_vault_map.get(vault_key)) {
                if let Ok(token_account) = TokenAccount::unpack(&account.data) {
                    if *is_base {
                        pools[*pool_idx].reserve_a = token_account.amount;
                    } else {
                        pools[*pool_idx].reserve_b = token_account.amount;
                    }
                }
            }
        }

        Ok(())
    }

    /// Пакетный запрос аккаунтов с разбивкой на чанки по 100
    fn get_multiple_accounts_batch(&self, keys: &[Pubkey]) -> Result<Vec<Option<solana_sdk::account::Account>>> {
        const BATCH_SIZE: usize = 100;
        let mut all_accounts = Vec::with_capacity(keys.len());

        for chunk in keys.chunks(BATCH_SIZE) {
            let accounts = self.rpc_client.get_multiple_accounts(chunk)?;
            all_accounts.extend(accounts);
        }

        Ok(all_accounts)
    }
}

#[async_trait::async_trait]
impl DexScanner for RaydiumAmmScanner {
    fn protocol(&self) -> DexProtocol {
        DexProtocol::RaydiumAmmV4
    }

    async fn scan_pools(&self) -> Result<Vec<PoolState>> {
        info!("📡 Начинаем параллельное сканирование Raydium AMM V4 пулов...");
        info!("   🎯 Program ID: {}", self.program_id);

        let config = RpcProgramAccountsConfig {
            filters: Some(vec![RpcFilterType::DataSize(752)]), // Размер AmmInfo
            account_config: RpcAccountInfoConfig {
                encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                data_slice: None,
                min_context_slot: None,
            },
            with_context: None,
            sort_results: None,
        };

        let accounts = self.rpc_client
            .get_program_accounts_with_config(&self.program_id, config)
            .context("Ошибка получения аккаунтов программы")?;

        info!("   📊 Найдено {} потенциальных аккаунтов пулов", accounts.len());

        // ПАРАЛЛЕЛЬНЫЙ парсинг с rayon
        let mut pools: Vec<PoolState> = accounts
            .par_iter()
            .filter_map(|(pubkey, account)| {
                match self.parse_pool_account_sync(*pubkey, &account.data) {
                    Ok(pool) => Some(pool),
                    Err(e) => {
                        debug!("⚠️ Не удалось распарсить пул {}: {}", pubkey, e);
                        None
                    }
                }
            })
            .collect();

        info!("✅ Успешно распарсено {} валидных пулов", pools.len());

        // ПАКЕТНОЕ получение резервов
        if !pools.is_empty() {
            self.fetch_vault_reserves_batch(&mut pools)?;

            // Фильтруем пулы с нулевыми резервами
            pools.retain(|pool| pool.reserve_a > 0 && pool.reserve_b > 0);

            info!("💰 Пулов с ненулевыми резервами: {}", pools.len());
        }

        Ok(pools)
    }

    fn clone_box(&self) -> Box<dyn DexScanner> {
        Box::new(self.clone())
    }
}