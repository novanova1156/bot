// bot/src/scanner/pool_monitor.rs
// Мониторинг изменений в пулах в реальном времени

// use anyhow::Result;
use dashmap::DashMap;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, debug};

use crate::types::PoolState;

/// Кэш состояний пулов с автообновлением
pub struct PoolMonitor {
    /// Кэш пулов: pool_id -> PoolState
    cache: Arc<DashMap<Pubkey, PoolState>>,

    /// Интервал обновления в миллисекундах
    update_interval_ms: u64,
}

impl PoolMonitor {
    pub fn new(update_interval_ms: u64) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            update_interval_ms,
        }
    }

    /// Обновление состояния пула
    pub fn update_pool(&self, pool: PoolState) {
        let pool_id = pool.id;

        // Проверяем значительность изменения
        if let Some(old_pool) = self.cache.get(&pool_id) {
            let price_change = self.calculate_price_change(&old_pool, &pool);

            if price_change > 0.5 {
                debug!("Значительное изменение цены в пуле {}: {:.2}%",
                       pool_id, price_change);
            }
        }

        self.cache.insert(pool_id, pool);
    }

    /// Получение всех пулов из кэша
    pub fn get_all_pools(&self) -> Vec<PoolState> {
        self.cache.iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Получение конкретного пула
    pub fn get_pool(&self, pool_id: &Pubkey) -> Option<PoolState> {
        self.cache.get(pool_id).map(|entry| entry.value().clone())
    }

    /// Количество пулов в кэше
    pub fn pool_count(&self) -> usize {
        self.cache.len()
    }

    /// Расчёт изменения цены в процентах
    fn calculate_price_change(&self, old: &PoolState, new: &PoolState) -> f64 {
        let old_price = old.price_a_to_b();
        let new_price = new.price_a_to_b();

        if old_price == 0.0 {
            return 0.0;
        }

        ((new_price - old_price) / old_price).abs() * 100.0
    }

    /// Очистка устаревших пулов (старше threshold_seconds)
    pub fn cleanup_stale_pools(&self, threshold_seconds: i64) {
        let now = chrono::Utc::now().timestamp();

        self.cache.retain(|_pool_id, pool| {
            let age = now - pool.last_updated;
            age < threshold_seconds
        });
    }
}

/// Фоновая задача периодической очистки кэша
pub async fn start_cache_cleanup_task(monitor: Arc<PoolMonitor>) {
    let mut cleanup_interval = interval(Duration::from_secs(60)); // Каждую минуту

    loop {
        cleanup_interval.tick().await;

        let count_before = monitor.pool_count();
        monitor.cleanup_stale_pools(300); // Удаляем старше 5 минут
        let count_after = monitor.pool_count();

        if count_before != count_after {
            info!("🧹 Очистка кэша: удалено {} устаревших пулов",
                  count_before - count_after);
        }
    }
}