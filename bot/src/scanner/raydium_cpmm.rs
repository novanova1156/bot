// bot/src/scanner/raydium_cpmm.rs
use anyhow::{Result, Context};
use rayon::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::RpcFilterType;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::{info, warn};

use crate::config::BotConfig;
use crate::types::{PoolState, DexProtocol};
use super::DexScanner;

#[derive(Clone)]
pub struct RaydiumCpmmScanner {
    rpc_client: Arc<RpcClient>,
    config: Arc<BotConfig>,
    program_id: Pubkey,
}

// Упрощенная CPMM структура
#[derive(Debug)]
struct CpmmPoolInfo {
    token_0_mint: Pubkey,
    token_1_mint: Pubkey,
    token_0_vault: Pubkey,
    token_1_vault: Pubkey,
    lp_supply: u64,
}

impl CpmmPoolInfo {
    fn try_from_slice(data: &[u8]) -> Result<Self> {
        if data.len() < 320 {
            return Err(anyhow::anyhow!("Недостаточно данных для CPMM pool"));
        }

        // Упрощенные офсеты для CPMM (нужно уточнить)
        let token_0_mint = crate::dex_structs::read_pubkey(data, 8)?;
        let token_1_mint = crate::dex_structs::read_pubkey(data, 40)?;
        let token_0_vault = crate::dex_structs::read_pubkey(data, 72)?;
        let token_1_vault = crate::dex_structs::read_pubkey(data, 104)?;
        let lp_supply = crate::dex_structs::read_u64(data, 200)?;

        Ok(Self {
            token_0_mint,
            token_1_mint,
            token_0_vault,
            token_1_vault,
            lp_supply,
        })
    }
}

impl RaydiumCpmmScanner {
    pub fn new(config: Arc<BotConfig>, rpc_client: Arc<RpcClient>) -> Result<Self> {
        let program_id = config.dex.raydium_cpmm.to_pubkey()
            .context("Некорректный Raydium CPMM program ID")?;

        info!("🔄 Инициализация Raydium CPMM сканера с program_id: {}", program_id);

        Ok(Self {
            rpc_client,
            config,
            program_id,
        })
    }

    fn parse_cpmm_pool(&self, pool_id: Pubkey, data: &[u8]) -> Result<PoolState> {
        let pool_info = CpmmPoolInfo::try_from_slice(data)?;

        Ok(PoolState {
            id: pool_id,
            protocol: DexProtocol::RaydiumCpmm,
            token_a: pool_info.token_0_mint,
            token_b: pool_info.token_1_mint,
            reserve_a: 0,
            reserve_b: 0,
            fee_bps: 25, // Типичная комиссия CPMM (0.25%)
            last_updated: chrono::Utc::now().timestamp(),
            full_state_data: data.to_vec(),
            decimals_a: 9,
            decimals_b: 9,
        })
    }
}

#[async_trait::async_trait]
impl DexScanner for RaydiumCpmmScanner {
    fn protocol(&self) -> DexProtocol {
        DexProtocol::RaydiumCpmm
    }

    async fn scan_pools(&self) -> Result<Vec<PoolState>> {
        info!("📡 Сканирование Raydium CPMM пулов...");

        let config = RpcProgramAccountsConfig {
            filters: Some(vec![
                RpcFilterType::DataSize(324), // Размер CPMM pool account
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
            .context("Ошибка получения CPMM аккаунтов")?;

        info!("   📊 Найдено {} потенциальных CPMM пулов", accounts.len());

        let pools: Vec<PoolState> = accounts
            .par_iter()
            .filter_map(|(pubkey, account)| {
                match self.parse_cpmm_pool(*pubkey, &account.data) {
                    Ok(pool) => Some(pool),
                    Err(e) => {
                        warn!("⚠️ Не удалось распарсить CPMM пул {}: {}", pubkey, e);
                        None
                    }
                }
            })
            .collect();

        info!("✅ Raydium CPMM: найдено {} пулов", pools.len());
        Ok(pools)
    }

    fn clone_box(&self) -> Box<dyn DexScanner> {
        Box::new(self.clone())
    }
}