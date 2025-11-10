// bot/src/scanner/mod.rs
pub mod pool_monitor; // ДОБАВЬТЕ ЭТУ СТРОКУ В НАЧАЛО
pub mod raydium_amm;
pub mod raydium_cpmm;
pub mod raydium_clmm;
pub mod meteora_dlmm;

use futures::future::join_all;
use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, warn, error};
use solana_client::rpc_client::RpcClient;

use crate::config::BotConfig;
use crate::types::{PoolState, DexProtocol};

use raydium_amm::RaydiumAmmScanner;
use raydium_cpmm::RaydiumCpmmScanner;
use raydium_clmm::RaydiumClmmScanner;
use meteora_dlmm::MeteoraDlmmScanner;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[async_trait]
pub trait DexScanner: Send + Sync {
    fn protocol(&self) -> DexProtocol;
    async fn scan_pools(&self) -> Result<Vec<PoolState>>;
    fn clone_box(&self) -> Box<dyn DexScanner>;
}

pub struct MultiDexScanner {
    scanners: Vec<Box<dyn DexScanner>>,
    config: Arc<BotConfig>,
    cached_test_pools: std::sync::Mutex<Option<Vec<PoolState>>>,
}

impl MultiDexScanner {
    pub fn new(config: Arc<BotConfig>, rpc_client: Arc<RpcClient>) -> Self {
        let scanners: Vec<Box<dyn DexScanner>> = vec![
            Box::new(RaydiumAmmScanner::new(config.clone(), rpc_client.clone()).unwrap()),
            Box::new(RaydiumCpmmScanner::new(config.clone(), rpc_client.clone()).unwrap()),
            Box::new(RaydiumClmmScanner::new(config.clone(), rpc_client.clone()).unwrap()),
            Box::new(MeteoraDlmmScanner::new(config.clone())), // [cite: 73]
        ];
        Self {
            scanners,
            config,
            cached_test_pools: std::sync::Mutex::new(None),
        }
    }

    /// НОВЫЙ МЕТОД: Установка тестовых пулов для devnet
    // ИСПРАВЛЕННЫЙ ТИП Vec<PoolState> (Ошибка 5)
    pub fn set_devnet_pools(&self, pools: Vec<PoolState>) { //
        let pools_count = pools.len();
        *self.cached_test_pools.lock().unwrap() = Some(pools);
        info!("🧪 Установлено {} готовых devnet пулов", pools_count);
    }

    // В методе scan_all_dex обновите сообщение:
    // ИСПРАВЛЕННЫЙ ТИП Result<Vec<PoolState>> (Ошибка 2)
    pub async fn scan_all_dex(&self) -> Result<Vec<PoolState>> { //
        let is_devnet = self.config.rpc.url.contains("devnet");
        if is_devnet { // [cite: 77]
            if let Some(devnet_pools) = self.cached_test_pools.lock().unwrap().as_ref() {
                if !devnet_pools.is_empty() {
                    info!("🧪 Devnet режим: используем {} готовых пулов", devnet_pools.len());
                    return Ok(devnet_pools.clone()); // [cite: 78]
                }
            }
            warn!("🧪 Devnet пулы не загружены, сканирование по сети"); // [cite: 79]
        }

        // ... (остальной код функции) [cite: 79-84]
        // ...
        let all_pools = Vec::new(); // [cite: 79]
        // ...
        if is_devnet && all_pools.is_empty() { // [cite: 81]
            if let Some(cached) = self.cached_test_pools.lock().unwrap().as_ref() {
                return Ok(cached.clone()); // [cite: 82]
            }
            error!("❌ В devnet режиме не найдено пулов для сканирования"); // [cite: 82]
            return Err(anyhow::anyhow!("No pools found in devnet")); // [cite: 82]
        }

        info!("📊 Найдено {} пулов в общем сканировании", all_pools.len()); // [cite: 83]
        Ok(all_pools) // [cite: 84]
    }
}