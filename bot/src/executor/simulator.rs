// bot/src/executor/simulator.rs (завершение)
use tracing::debug;
use solana_sdk::transaction::Transaction;
use crate::types::SimulationResult;
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use std::sync::Arc;
pub struct TransactionSimulator {
    rpc_client: Arc<RpcClient>,
}
/// Симуляция транзакции
impl TransactionSimulator {
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client }
    }

    pub async fn simulate(&self, transaction: &Transaction) -> Result<SimulationResult> {
        let simulation = self.rpc_client
            .simulate_transaction(transaction)
            .map_err(|e| anyhow::anyhow!("Ошибка симуляции: {}", e))?;

        let result = SimulationResult {
            err: simulation.value.err.map(|e| format!("{:?}", e)),
            logs: simulation.value.logs.clone().unwrap_or_default(),
            units_consumed: simulation.value.units_consumed,
        };

        if let Some(ref err) = result.err {
            debug!("Симуляция завершилась с ошибкой: {}", err);
            if let Some(logs) = &simulation.value.logs {
                for log in logs {
                    debug!("  Log: {}", log);
                }
            }
        }

        Ok(result)
    }

    /// Оценка compute units для транзакции
    pub async fn estimate_compute_units(&self, transaction: &Transaction) -> Result<u64> {
        let simulation = self.simulate(transaction).await?;

        simulation.units_consumed
            .ok_or_else(|| anyhow::anyhow!("Не удалось определить compute units"))
    }
}