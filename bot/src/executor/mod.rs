// bot/src/executor/mod.rs

pub mod transaction_builder;
pub mod jito_client;
pub mod simulator;

use anyhow::Result;
use solana_sdk::signature::{Keypair, Signature};
use solana_client::rpc_client::RpcClient;
use std::sync::Arc;
use tracing::info;

use crate::config::BotConfig;
use crate::types::ArbitrageOpportunity;
use transaction_builder::TransactionBuilder;
use simulator::TransactionSimulator;

pub struct TransactionExecutor {
    rpc_client: Arc<RpcClient>,
    builder: TransactionBuilder,
    simulator: TransactionSimulator,
}

impl TransactionExecutor {
    pub fn new(
        rpc_client: Arc<RpcClient>,
        keypair: Arc<Keypair>,
        config: Arc<BotConfig>,
    ) -> Result<Self> {
        Ok(Self {
            builder: TransactionBuilder::new(
                rpc_client.clone(),
                keypair.clone(),
                config.clone(),
            )?,
            simulator: TransactionSimulator::new(rpc_client.clone()),
            rpc_client,
        })
    }

    pub async fn execute(&self, opportunity: &ArbitrageOpportunity) -> Result<Signature> {
        let transaction = self.builder.build_arbitrage_transaction(opportunity).await?;

        info!("🧪 Симуляция транзакции...");
        let simulation = self.simulator.simulate(&transaction).await?;
        if let Some(err) = simulation.err {
            anyhow::bail!("Симуляция провалилась: {}\nЛоги:\n{:#?}", err, simulation.logs);
        }

        info!("✅ Симуляция успешна (CU: {})", simulation.units_consumed.unwrap_or(0));

        info!("📤 Отправка транзакции...");
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(signature)
    }
}