// bot/src/config.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub rpc: RpcConfig,
    pub wallet: WalletConfig,
    pub trading: TradingConfig,
    pub dex: DexConfig,
    pub jito: Option<JitoConfig>,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub url: String,
    pub ws_url: String,
    pub commitment: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub executor_program_id: PubkeyString,
    pub min_profit_lamports: u64,
    pub min_profit_bps: u16,
    pub max_slippage_bps: u16,
    pub initial_amount_sol: f64,
    pub max_legs: u8,
    pub compute_unit_limit: u32,
    pub priority_fee_micro_lamports: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    pub raydium_amm_v4: PubkeyString,
    pub raydium_cpmm: PubkeyString,
    pub raydium_clmm: PubkeyString,
    pub meteora_dlmm: PubkeyString,
    pub openbook_id: PubkeyString,  // НОВОЕ ПОЛЕ
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoConfig {
    pub block_engine_url: String,
    pub tip_account: PubkeyString,
    pub tip_lamports: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub log_level: String,
    pub telemetry_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubkeyString(pub String);

impl PubkeyString {
    pub fn to_pubkey(&self) -> Result<Pubkey> {
        Pubkey::from_str(&self.0).context("Invalid pubkey")
    }
}

impl BotConfig {
    pub fn load() -> Result<Self> {
        dotenv::dotenv().ok();

        // ПОДДЕРЖКА КЛАСТЕРОВ
        let cluster = std::env::var("SOLANA_CLUSTER").unwrap_or_else(|_| "mainnet".to_string());
        let is_devnet = cluster.eq_ignore_ascii_case("devnet");

        let (rpc_url, ws_url) = if is_devnet {
            (
                "https://api.devnet.solana.com".to_string(),
                "wss://api.devnet.solana.com".to_string(),
            )
        } else {
            (
                std::env::var("SOLANA_RPC_URL")
                    .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
                std::env::var("SOLANA_WS_URL")
                    .unwrap_or_else(|_| "wss://api.mainnet-beta.solana.com".to_string()),
            )
        };

        // ПРАВИЛЬНЫЕ PROGRAM IDs ДЛЯ DEVNET/MAINNET
        let dex = if is_devnet {
            DexConfig {
                raydium_amm_v4: PubkeyString("DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav".to_string()),
                raydium_cpmm: PubkeyString("DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb".to_string()),
                raydium_clmm: PubkeyString("DRayAUgENGQBKVaX8owNhgzkEDyoHTGVEGHVJT1E9pfH".to_string()),
                meteora_dlmm: PubkeyString("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo".to_string()),
                openbook_id: PubkeyString("opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb".to_string()),
            }
        } else {
            DexConfig {
                raydium_amm_v4: PubkeyString("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string()),
                raydium_cpmm: PubkeyString("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C".to_string()),
                raydium_clmm: PubkeyString("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string()),
                meteora_dlmm: PubkeyString("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo".to_string()),
                openbook_id: PubkeyString("srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX".to_string()),
            }
        };

        Ok(Self {
            rpc: RpcConfig {
                url: rpc_url,
                ws_url,
                commitment: "confirmed".to_string(),
                timeout_seconds: 30,
            },
            wallet: WalletConfig {
                path: std::env::var("WALLET_PATH")
                    .unwrap_or_else(|_| "~/.config/solana/id.json".to_string())
                    .into(),
            },
            trading: TradingConfig {
                executor_program_id: PubkeyString(
                    std::env::var("ARBITRAGE_EXECUTOR_PROGRAM_ID")
                        .context("ARBITRAGE_EXECUTOR_PROGRAM_ID не найден в .env")?,
                ),
                min_profit_lamports: std::env::var("MIN_PROFIT_LAMPORTS")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .context("Invalid MIN_PROFIT_LAMPORTS")?,
                min_profit_bps: std::env::var("MIN_PROFIT_BPS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .context("Invalid MIN_PROFIT_BPS")?,
                max_slippage_bps: std::env::var("MAX_SLIPPAGE_BPS")
                    .unwrap_or_else(|_| "500".to_string())
                    .parse()
                    .context("Invalid MAX_SLIPPAGE_BPS")?,
                initial_amount_sol: std::env::var("INITIAL_AMOUNT_SOL")
                    .unwrap_or_else(|_| "0.01".to_string())
                    .parse()
                    .context("Invalid INITIAL_AMOUNT_SOL")?,
                max_legs: 5,
                compute_unit_limit: 400_000,
                priority_fee_micro_lamports: 100_000,
            },
            dex,
            jito: None, // Отключаем Jito на devnet
            monitoring: MonitoringConfig {
                log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
                telemetry_enabled: std::env::var("TELEMETRY_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
            },
        })
    }
}