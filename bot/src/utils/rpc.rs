// bot/src/utils/rpc.rs
// Утилиты для работы с RPC клиентами Solana

use anyhow::Result;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
    // УДАЛИТЕ неиспользуемые импорты:
    // rpc_config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig},
    // client_error::ClientError,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    transaction::Transaction,
    signature::Signature,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{warn, debug};

/// Конфигурация ретраев для RPC запросов
pub struct RetryConfig {
    pub max_retries: usize,
    pub base_delay_ms: u64,
    pub exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 500,
            exponential_backoff: true,
        }
    }
}

/// RPC клиент с множественными endpoints и ретраями
pub struct MultiRpcClient {
    primary: RpcClient,
    fallbacks: Vec<RpcClient>,
    retry_config: RetryConfig,
}

impl MultiRpcClient {
    pub fn new(primary_url: String, fallback_urls: Vec<String>) -> Self {
        let primary = RpcClient::new_with_commitment(
            primary_url,
            CommitmentConfig::confirmed(),
        );

        let fallbacks: Vec<RpcClient> = fallback_urls
            .into_iter()
            .map(|url| RpcClient::new_with_commitment(url, CommitmentConfig::confirmed()))
            .collect();

        Self {
            primary,
            fallbacks,
            retry_config: RetryConfig::default(),
        }
    }

    /// Отправка транзакции с ретраями
    pub async fn send_transaction_with_retry(
        &self,
        transaction: &Transaction,
    ) -> Result<Signature> {
        let config = RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
            ..Default::default()
        };

        let mut last_error = None;

        // Попытка через primary RPC
        for attempt in 0..self.retry_config.max_retries {
            match self.primary.send_transaction_with_config(transaction, config) {
                Ok(signature) => {
                    debug!("Транзакция отправлена: {} (попытка {})", signature, attempt + 1);
                    return Ok(signature);
                }
                Err(e) => {
                    warn!("Ошибка отправки (попытка {}): {}", attempt + 1, e);
                    last_error = Some(e);

                    if attempt < self.retry_config.max_retries - 1 {
                        let delay = self.calculate_delay(attempt);
                        sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        // Попытка через fallback RPCs
        for (idx, fallback) in self.fallbacks.iter().enumerate() {
            match fallback.send_transaction_with_config(transaction, config) {
                Ok(signature) => {
                    debug!("Транзакция отправлена через fallback #{}: {}", idx + 1, signature);
                    return Ok(signature);
                }
                Err(e) => {
                    warn!("Fallback #{} провалился: {}", idx + 1, e);
                    last_error = Some(e);
                }
            }
        }

        Err(anyhow::anyhow!(
            "Не удалось отправить транзакцию после {} попыток: {:?}",
            self.retry_config.max_retries,
            last_error
        ))
    }

    /// Симуляция транзакции с ретраями
    pub async fn simulate_transaction_with_retry(
        &self,
        transaction: &Transaction,
    ) -> Result<solana_client::rpc_response::RpcSimulateTransactionResult> {
        let mut last_error = None;

        for attempt in 0..self.retry_config.max_retries {
            match self.primary.simulate_transaction(transaction) {
                Ok(result) => return Ok(result.value),
                Err(e) => {
                    warn!("Симуляция провалилась (попытка {}): {}", attempt + 1, e);
                    last_error = Some(e);

                    if attempt < self.retry_config.max_retries - 1 {
                        let delay = self.calculate_delay(attempt);
                        sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "Симуляция провалилась после {} попыток: {:?}",
            self.retry_config.max_retries,
            last_error
        ))
    }

    /// Расчёт задержки с exponential backoff
    fn calculate_delay(&self, attempt: usize) -> u64 {
        if self.retry_config.exponential_backoff {
            self.retry_config.base_delay_ms * (2_u64.pow(attempt as u32))
        } else {
            self.retry_config.base_delay_ms
        }
    }
}