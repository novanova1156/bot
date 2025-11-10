// bot/src/scanner/raydium_clmm.rs
use anyhow::{Result, Context};
use rayon::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{RpcFilterType, Memcmp, MemcmpEncodedBytes};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::{info, warn};

use crate::config::BotConfig;
use crate::types::{PoolState, DexProtocol};
use super::DexScanner;

#[derive(Clone)]
pub struct RaydiumClmmScanner {
    rpc_client: Arc<RpcClient>,
    config: Arc<BotConfig>,
    program_id: Pubkey,
}

// Упрощенная CLMM структура для парсинга ключевых полей
#[derive(Debug)]
struct ClmmPoolInfo {
    token_mint_0: Pubkey,
    token_mint_1: Pubkey,
    token_vault_0: Pubkey,
    token_vault_1: Pubkey,
    tick_spacing: u16,
    liquidity: u128,
}

impl ClmmPoolInfo {
    /// Парсинг CLMM pool из сырых данных (упрощенная версия)
    fn try_from_slice(data: &[u8]) -> Result<Self> {
        if data.len() < 400 {
            return Err(anyhow::anyhow!("Недостаточно данных для CLMM pool"));
        }

        // Примерные офсеты для CLMM (нужно уточнить по IDL)
        let token_mint_0 = crate::dex_structs::read_pubkey(data, 72)?;
        let token_mint_1 = crate::dex_structs::read_pubkey(data, 104)?;
        let token_vault_0 = crate::dex_structs::read_pubkey(data, 136)?;
        let token_vault_1 = crate::dex_structs::read_pubkey(data, 168)?;
        let tick_spacing = u16::from_le_bytes([data[200], data[201]]);
        let liquidity = u128::from_le_bytes(
            data[300..316].try_into().unwrap_or([0u8; 16])
        );

        Ok(Self {
            token_mint_0,
            token_mint_1,
            token_vault_0,
            token_vault_1,
            tick_spacing,
            liquidity,
        })
    }
}

impl RaydiumClmmScanner {
    pub fn new(config: Arc<BotConfig>, rpc_client: Arc<RpcClient>) -> Result<Self> {
        let program_id = config.dex.raydium_clmm.to_pubkey()
            .context("Некорректный Raydium CLMM program ID")?;

        info!("🌊 Инициализация Raydium CLMM сканера с program_id: {}", program_id);

        Ok(Self {
            rpc_client,
            config,
            program_id,
        })
    }

    fn parse_clmm_pool(&self, pool_id: Pubkey, data: &[u8]) -> Result<PoolState> {
        let pool_info = ClmmPoolInfo::try_from_slice(data)?;

        Ok(PoolState {
            id: pool_id,
            protocol: DexProtocol::RaydiumClmm,
            token_a: pool_info.token_mint_0,
            token_b: pool_info.token_mint_1,
            reserve_a: 0, // Будет получено из vault'ов
            reserve_b: 0,
            fee_bps: 30, // Типичная комиссия CLMM (0.3%)
            last_updated: chrono::Utc::now().timestamp(),
            full_state_data: data.to_vec(),
            decimals_a: 9,
            decimals_b: 9,
        })
    }
}

#[async_trait::async_trait]
impl DexScanner for RaydiumClmmScanner {
    fn protocol(&self) -> DexProtocol {
        DexProtocol::RaydiumClmm
    }

    async fn scan_pools(&self) -> Result<Vec<PoolState>> {
        info!("📡 Сканирование Raydium CLMM пулов...");

        // Фильтры для поиска CLMM пулов
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![
                RpcFilterType::DataSize(1544), // Размер CLMM pool account
            ]),
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
            .context("Ошибка получения CLMM аккаунтов")?;

        info!("   📊 Найдено {} потенциальных CLMM пулов", accounts.len());

        // Параллельный парсинг
        let pools: Vec<PoolState> = accounts
            .par_iter()
            .filter_map(|(pubkey, account)| {
                match self.parse_clmm_pool(*pubkey, &account.data) {
                    Ok(pool) => Some(pool),
                    Err(e) => {
                        warn!("⚠️ Не удалось распарсить CLMM пул {}: {}", pubkey, e);
                        None
                    }
                }
            })
            .collect();

        info!("✅ Raydium CLMM: найдено {} пулов", pools.len());
        Ok(pools)
    }

    fn clone_box(&self) -> Box<dyn DexScanner> {
        Box::new(self.clone())
    }
}