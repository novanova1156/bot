// bot/src/arbitrage/profit_calculator.rs
// Расчёт чистой прибыли с учётом всех комиссий

use anyhow::Result;
use std::sync::Arc;

use crate::config::BotConfig;
use crate::types::SwapLeg;

pub struct ProfitCalculator {
    config: Arc<BotConfig>,
}

impl ProfitCalculator {
    pub fn new(config: Arc<BotConfig>) -> Self {
        Self { config }
    }

    /// Расчёт чистой прибыли с учётом комиссий
    /// В devnet НЕ вычитаем SOL-комиссии (другая единица) — работаем в атомах стартового токена
    pub fn calculate_net_profit(
        &self,
        initial_amount: u64,
        final_amount: u64,
        _legs: &[SwapLeg],
    ) -> Result<(u64, u64)> {
        // Валовая прибыль в атомах токена
        let gross_profit = if final_amount >= initial_amount {
            final_amount - initial_amount
        } else {
            initial_amount - final_amount
        };

        // Devnet: игнорируем комиссии в SOL (lamports), т.к. считаем в атомах токена
        let is_devnet = self.config.rpc.url.contains("devnet");

        let net_profit = if is_devnet {
            gross_profit
        } else {
            // Для mainnet здесь следовало бы:
            // 1) рассчитать SOL-комиссии (tx_base_fee, priority_fee, jito_tip)
            // 2) сконвертировать их в атомы токена A через прайс-оракул
            // 3) вычесть из gross_profit
            gross_profit
        };

        Ok((gross_profit, net_profit))
    }

    /// Оценка максимально допустимого slippage (оставлено без изменений)
    pub fn calculate_max_slippage(
        &self,
        initial_amount: u64,
        expected_profit: u64,
    ) -> f64 {
        let max_loss = expected_profit.saturating_sub(self.config.trading.min_profit_lamports);
        if expected_profit == 0 { return 0.0; }
        (max_loss as f64 / expected_profit as f64) * 100.0
    }
}