// bot/src/main.rs
// Точка входа арбитражного бота
mod devnet_pools;
mod config;
mod types;
mod scanner;
mod arbitrage;
mod executor;
mod utils;
pub mod dex_structs;

use solana_sdk::signature::Signer;
use anyhow::{Result, Context};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, error, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::BotConfig;
use scanner::{MultiDexScanner, pool_monitor::PoolMonitor};
use arbitrage::ArbitrageFinder;
use executor::TransactionExecutor;
use utils::load_keypair_from_file;
use devnet_pools::get_devnet_pools;

#[tokio::main]
async fn main() -> Result<()> {
    // Инициализация логирования
    init_logging();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║    SOLANA ARBITRAGE BOT - RUST EDITION (DEVNET)               ║");
    println!("║    Высокопроизводительный поиск и исполнение арбитража        ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Загрузка конфигурации
    info!("📋 Загрузка конфигурации...");
    let config = Arc::new(BotConfig::load()?);
    info!("✅ Конфигурация загружена");

    // Загрузка кошелька
    info!("🔑 Загрузка кошелька...");
    let wallet_path = config.wallet.path.to_str()
        .ok_or_else(|| anyhow::anyhow!("Неверный путь к кошельку"))?;
    let keypair = Arc::new(load_keypair_from_file(wallet_path)?);
    info!("   Публичный ключ: {}", keypair.pubkey());

    // Инициализация RPC клиента
    info!("🌐 Подключение к Solana RPC...");
    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        config.rpc.url.clone(),
        CommitmentConfig::confirmed(),
    ));

    // Проверка подключения
    let cluster_version = rpc_client.get_version()?;
    info!("   Подключено к кластеру: {} (Solana {})",
          config.rpc.url, cluster_version.solana_core);

    // Проверка баланса
    let balance = rpc_client.get_balance(&keypair.pubkey())?;
    let balance_sol = balance as f64 / 1_000_000_000.0;
    info!("💰 Баланс кошелька: {:.9} SOL", balance_sol);

    if balance_sol < 0.1 {
        warn!("⚠️  НИЗКИЙ БАЛАНС! Пополните через фоссет: https://faucet.solana.com/");
        warn!("   Адрес: {}", keypair.pubkey());
    }

    // Инициализация компонентов
    info!("🔧 Инициализация компонентов бота...");

    let pool_monitor = Arc::new(PoolMonitor::new(5000)); // 5 секунд TTL
    let dex_scanner = MultiDexScanner::new(config.clone(), rpc_client.clone());
    let arbitrage_finder = ArbitrageFinder::new(config.clone());
    let executor = TransactionExecutor::new(
        rpc_client.clone(),
        keypair.clone(),
        config.clone(),
    )?;

    info!("✅ Все компоненты инициализированы\n");

    // СОЗДАНИЕ ТЕСТОВОЙ СРЕДЫ для devnet (TS-скрипты можно добавить позже,
    // здесь реализация fallback на Rust гарантированно создаст 3 токена и 3 пула)
    if config.rpc.url.contains("devnet") {
        info!("🧪 Режим devnet: загрузка готовых пулов");

        // ИСПРАВЛЕННЫЙ БЛОК: Заменяем 'match' на 'let ... ?'
        // для автоматического преобразования типов ошибок
        let pools = get_devnet_pools()
            .context("❌ Ошибка загрузки devnet пулов")?; // [cite: 132, 137]

        info!("✅ Загружено {} готовых devnet пулов", pools.len());
        // Список пулов
        for (i, pool) in pools.iter().enumerate() {
            info!(
                "   Пул #{}: {} ({:?})",
                i + 1,
                pool.id.to_string(),
                pool.protocol
            );
        }

        // Установка пулов в сканер
        dex_scanner.set_devnet_pools(pools); // [cite: 135]
    }
    // Запуск фоновой очистки кэша
    let monitor_clone = pool_monitor.clone();
    tokio::spawn(async move {
        scanner::pool_monitor::start_cache_cleanup_task(monitor_clone).await;
    });

    // Главный цикл бота
    info!("🚀 Запуск главного цикла бота...");
    info!("{}", "═".repeat(80));

    let mut scan_interval = interval(Duration::from_millis(config.rpc.timeout_seconds * 1000));
    let mut iteration = 0u64;

    loop {
        scan_interval.tick().await;
        iteration += 1;

        info!("\n⏰ Итерация #{} - {}", iteration, chrono::Local::now().format("%H:%M:%S"));

        // Шаг 1: Сканирование пулов
        match dex_scanner.scan_all_dex().await {
            Ok(pools) => {
                info!("📊 Загружено {} пулов для арбитража", pools.len());
                if pools.is_empty() {
                    warn!("   ⚠️  Пулы не найдены. Убедитесь что тестовые пулы созданы на devnet.");
                    continue;
                }

                // Обновление кэша
                for pool in &pools {
                    pool_monitor.update_pool(pool.clone());
                }

                info!("   📊 Активных пулов: {}", pools.len());

                // Шаг 2: Поиск арбитражных возможностей
                match arbitrage_finder.find_opportunities(&pools) {
                    Ok(opportunities) => {
                        if opportunities.is_empty() {
                            info!("   ⏳ Прибыльных возможностей не найдено");
                            continue;
                        }

                        info!("   🔥 Найдено возможностей: {}", opportunities.len());

                        // Берём лучшую возможность
                        let best = &opportunities[0];
                        info!("   💎 Лучшая возможность:");
                        info!("      Прибыль: {:.9} SOL ({:.4}%)",
                              best.net_profit as f64 / 1_000_000_000.0,
                              best.profit_percentage);
                        info!("      Шагов: {}", best.legs.len());

                        // Шаг 3: Исполнение арбитража
                        info!("   🔧 Исполнение арбитража...");
                        match executor.execute(best).await {
                            Ok(signature) => {
                                info!("   ✅ АРБИТРАЖ УСПЕШЕН!");
                                info!("      Транзакция: {}", signature);
                                info!("      Explorer: https://explorer.solana.com/tx/{}?cluster=devnet",
                                      signature);
                            }
                            Err(e) => {
                                error!("   ❌ Ошибка исполнения: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("   ❌ Ошибка поиска возможностей: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("   ❌ Ошибка сканирования пулов: {}", e);
            }
        }
    }
}

/// Инициализация системы логирования
fn init_logging() {
    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_thread_ids(false)
            .with_line_number(false))
        .init();
}