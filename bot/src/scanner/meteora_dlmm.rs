// bot/src/scanner/meteora_dlmm.rs
use anyhow::Result;
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::{info, warn};
use std::sync::Arc;

use crate::config::BotConfig;
use crate::types::{PoolState, DexProtocol};
use super::DexScanner;

#[derive(Clone)]
pub struct MeteoraDlmmScanner {
    config: Arc<BotConfig>,
}

#[derive(Deserialize)]
struct MeteoraPair {
    address: String,
    mint_x: String,
    mint_y: String,
    reserve_x_amount: u64,
    reserve_y_amount: u64,
    base_fee_percentage: String,
}

impl MeteoraDlmmScanner {
    pub fn new(config: Arc<BotConfig>) -> Self {
        Self { config }
    }

    fn convert_api_pool_to_pool_state(&self, api_pool: MeteoraPair) -> Result<PoolState> {
        let id = Pubkey::from_str(&api_pool.address)?;
        let token_a = Pubkey::from_str(&api_pool.mint_x)?;
        let token_b = Pubkey::from_str(&api_pool.mint_y)?;

        let fee_pct: f64 = api_pool.base_fee_percentage.parse()?;
        let fee_bps = (fee_pct * 100.0) as u16;

        Ok(PoolState {
            id,
            protocol: DexProtocol::MeteoraDlmm,
            token_a,
            token_b,
            reserve_a: api_pool.reserve_x_amount,
            reserve_b: api_pool.reserve_y_amount,
            fee_bps,
            last_updated: chrono::Utc::now().timestamp(),
            full_state_data: Vec::new(),
            decimals_a: 9,
            decimals_b: 9,
        })
    }
}

#[async_trait::async_trait]
impl DexScanner for MeteoraDlmmScanner {
    fn protocol(&self) -> DexProtocol {
        DexProtocol::MeteoraDlmm
    }

    async fn scan_pools(&self) -> Result<Vec<PoolState>> {
        info!("📡 Сканирование Meteora DLMM пулов через API...");

        // КРИТИЧЕСКОЕ ИСПРАВЛЕНИЕ: добавляем параметр кластера для devnet
        let api_url = "https://dlmm-api.meteora.ag/pair/all?cluster=devnet";

        let response = reqwest::get(api_url).await?;

        if !response.status().is_success() {
            anyhow::bail!("API Meteora вернуло ошибку: {}", response.status());
        }

        let api_pools: Vec<MeteoraPair> = response.json().await?;

        info!("   📊 Получено {} пулов от API Meteora (devnet)", api_pools.len());

        let mut pools = Vec::new();
        for api_pool in api_pools {
            match self.convert_api_pool_to_pool_state(api_pool) {
                Ok(pool) => pools.push(pool),
                Err(e) => warn!("⚠️ Ошибка конвертации пула Meteora: {}", e),
            }
        }

        info!("✅ Meteora DLMM: найдено {} валидных пулов", pools.len());
        Ok(pools)
    }

    fn clone_box(&self) -> Box<dyn DexScanner> {
        Box::new(self.clone())
    }
}