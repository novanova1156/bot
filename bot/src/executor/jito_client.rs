// bot/src/executor/jito_client.rs
// Клиент для интеграции с Jito Block Engine (MEV защита)

// bot/src/executor/jito_client.rs

use anyhow::{Result, Context};
use solana_sdk::{
    pubkey::Pubkey,
    transaction::Transaction,
    // УДАЛИТЕ эту строку: signature::Signature,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, warn, debug};

// УДАЛИТЕ эту строку - она дублирует импорт:
// use solana_sdk::pubkey::Pubkey;

/// Конфигурация Jito
pub struct JitoConfig {
    pub block_engine_url: String,
    pub tip_account: Pubkey,
    pub tip_lamports: u64,
}

impl Default for JitoConfig {
    fn default() -> Self {
        Self {
            block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
            tip_account: Pubkey::from_str("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5")
                .expect("Invalid Jito tip account"),
            tip_lamports: 300_000, // 0.0003 SOL минимальный tip
        }
    }
}

/// Клиент Jito Block Engine
pub struct JitoClient {
    config: JitoConfig,
    http_client: Client,
}

impl JitoClient {
    pub fn new(config: JitoConfig) -> Self {
        Self {
            config,
            http_client: Client::new(),
        }
    }

    /// Отправка bundle транзакций в Jito
    ///
    /// ВАЖНО: Jito Block Engine недоступен на devnet!
    /// Эта функция работает только на mainnet.
    pub async fn send_bundle(&self, transactions: Vec<Transaction>) -> Result<String> {
        // Проверка devnet
        if self.config.block_engine_url.contains("devnet") {
            warn!("⚠️  Jito Block Engine недоступен на devnet. Используйте обычную отправку.");
            anyhow::bail!("Jito не поддерживается на devnet");
        }

        info!("📦 Отправка bundle из {} транзакций в Jito...", transactions.len());

        // Сериализация транзакций в base64
        let encoded_txs: Vec<String> = transactions
            .iter()
            .map(|tx| {
                let serialized = bincode::serialize(tx).expect("Failed to serialize tx");
                bs58::encode(serialized).into_string()
            })
            .collect();

        debug!("   Сериализовано {} транзакций", encoded_txs.len());

        // Подготовка JSON-RPC запроса
        let request = SendBundleRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "sendBundle".to_string(),
            params: vec![encoded_txs],
        };

        // Отправка POST запроса к Jito API
        let endpoint = format!("{}/api/v1/bundles", self.config.block_engine_url);
        debug!("   Endpoint: {}", endpoint);

        let response = self.http_client
            .post(&endpoint)
            .json(&request)
            .send()
            .await
            .context("Не удалось отправить bundle в Jito")?;

        // Проверка статуса ответа
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Jito вернул ошибку {}: {}", status, error_text);
        }

        // Парсинг ответа
        let bundle_response: SendBundleResponse = response
            .json()
            .await
            .context("Не удалось распарсить ответ Jito")?;

        if let Some(error) = bundle_response.error {
            anyhow::bail!("Jito RPC ошибка: {:?}", error);
        }

        let bundle_id = bundle_response.result
            .ok_or_else(|| anyhow::anyhow!("Нет bundle_id в ответе Jito"))?;

        info!("   ✅ Bundle отправлен, ID: {}", bundle_id);

        Ok(bundle_id)
    }

    /// Проверка статуса bundle
    pub async fn get_bundle_status(&self, bundle_id: &str) -> Result<BundleStatus> {
        let request = GetBundleStatusRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "getBundleStatuses".to_string(),
            params: vec![vec![bundle_id.to_string()]],
        };

        let endpoint = format!("{}/api/v1/bundles", self.config.block_engine_url);

        let response = self.http_client
            .post(&endpoint)
            .json(&request)
            .send()
            .await
            .context("Не удалось получить статус bundle")?;

        let status_response: GetBundleStatusResponse = response
            .json()
            .await
            .context("Не удалось распарсить статус bundle")?;

        let statuses = status_response.result.value
            .ok_or_else(|| anyhow::anyhow!("Нет статусов в ответе"))?;

        let status = statuses.first()
            .ok_or_else(|| anyhow::anyhow!("Пустой массив статусов"))?;

        Ok(status.clone())
    }

    /// Ожидание подтверждения bundle (с таймаутом)
    pub async fn wait_for_confirmation(
        &self,
        bundle_id: &str,
        timeout_seconds: u64,
    ) -> Result<BundleStatus> {
        use tokio::time::{sleep, Duration};

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_seconds);

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Таймаут ожидания подтверждения bundle");
            }

            let status = self.get_bundle_status(bundle_id).await?;

            match status.confirmation_status.as_str() {
                "confirmed" | "finalized" => {
                    info!("   ✅ Bundle подтверждён: {}", status.confirmation_status);
                    return Ok(status);
                }
                "failed" => {
                    anyhow::bail!("Bundle провалился: {:?}", status.err);
                }
                "pending" => {
                    debug!("   Bundle в ожидании... ({}s)", start.elapsed().as_secs());
                    sleep(Duration::from_millis(500)).await;
                }
                _ => {
                    debug!("   Неизвестный статус: {}", status.confirmation_status);
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
}

// ============================================================================
// JSON-RPC СТРУКТУРЫ
// ============================================================================

#[derive(Serialize)]
struct SendBundleRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<Vec<String>>,
}

#[derive(Deserialize)]
struct SendBundleResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct GetBundleStatusRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<Vec<String>>,
}

#[derive(Deserialize)]
struct GetBundleStatusResponse {
    result: BundleStatusResult,
}

#[derive(Deserialize)]
struct BundleStatusResult {
    value: Option<Vec<BundleStatus>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BundleStatus {
    pub bundle_id: String,
    pub confirmation_status: String,
    pub err: Option<String>,
    pub slot: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// ============================================================================
// ВСПОМОГАТЕЛЬНЫЕ ФУНКЦИИ
// ============================================================================

/// Создание tip транзакции для Jito
pub fn create_tip_instruction(
    from: &Pubkey,
    tip_account: &Pubkey,
    lamports: u64,
) -> solana_sdk::instruction::Instruction {
    solana_sdk::system_instruction::transfer(from, tip_account, lamports)
}