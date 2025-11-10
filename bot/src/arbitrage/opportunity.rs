// bot/src/arbitrage/opportunity.rs
// Оценка и валидация арбитражных возможностей

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

use solana_sdk::pubkey::Pubkey;

use super::bellman_ford::ArbitrageCycle;
use super::profit_calculator::ProfitCalculator;
use crate::config::BotConfig;
use crate::types::{ArbitrageOpportunity, DexProtocol, PoolState, SwapLeg};

pub struct OpportunityEvaluator {
    config: Arc<BotConfig>,
    profit_calc: ProfitCalculator,
}

impl OpportunityEvaluator {
    pub fn new(config: Arc<BotConfig>) -> Self {
        Self {
            profit_calc: ProfitCalculator::new(config.clone()),
            config,
        }
    }

    /// Строим таблицу decimals по mint-адресам из списка пулов.
    fn build_decimals_map(&self, pools: &[PoolState]) -> HashMap<Pubkey, u8> {
        let mut m = HashMap::new();
        for p in pools {
            // Заполняем только если не было значений ранее
            m.entry(p.token_a).or_insert(p.decimals_a);
            m.entry(p.token_b).or_insert(p.decimals_b);
        }
        m
    }

    /// Оценка цикла и создание ArbitrageOpportunity
    pub fn evaluate_cycle(
        &self,
        cycle: &ArbitrageCycle,
        pools: &[PoolState],
    ) -> Result<Option<ArbitrageOpportunity>> {
        // Минимум три токена (A -> B -> C -> A)
        if cycle.tokens.len() < 3 {
            return Ok(None);
        }

        info!("🔍 === ДЕТАЛЬНАЯ ДИАГНОСТИКА ЦИКЛА ===");
        info!("Токенов в цикле: {}", cycle.tokens.len());

        // Построим карту decimals из пулов
        let decimals_map = self.build_decimals_map(pools);

        // Начальная сумма: интерпретируем initial_amount_sol как количество в UI-единицах
        // стартового токена и переводим в атомы стартового токена.
        let start_mint = cycle.tokens[0];
        let start_decimals = *decimals_map
            .get(&start_mint)
            .ok_or_else(|| anyhow::anyhow!("Не найдены decimals для стартового токена"))?;
        let ui_amount = self.config.trading.initial_amount_sol; // используем как UI количество
        let mut current_amount: u64 =
            (ui_amount * 10f64.powi(start_decimals as i32)) as u64;

        info!(
            "💰 Начальная сумма: {} atoms (mint: {}, decimals: {})",
            current_amount, start_mint, start_decimals
        );

        // Построение swap legs с детальным логированием
        let mut legs: Vec<SwapLeg> = Vec::new();

        for i in 0..cycle.tokens.len() - 1 {
            let input_mint = cycle.tokens[i];
            let output_mint = cycle.tokens[i + 1];

            info!("🔄 === СВОП #{} ===", i + 1);
            info!("От: {}", input_mint);
            info!("К:  {}", output_mint);

            // Находим пул для этой пары
            let pool = pools
                .iter()
                .find(|p| {
                    (p.token_a == input_mint && p.token_b == output_mint)
                        || (p.token_a == output_mint && p.token_b == input_mint)
                })
                .ok_or_else(|| anyhow::anyhow!("Пул не найден для пары токенов"))?;

            info!("📊 Найден пул: {}", pool.id);
            info!(
                "   Token A: {} (резерв: {} atoms, decimals: {}), Token B: {} (резерв: {} atoms, decimals: {})",
                pool.token_a, pool.reserve_a, pool.decimals_a, pool.token_b, pool.reserve_b, pool.decimals_b
            );

            // Определяем направление свопа
            let a_to_b = input_mint == pool.token_a;
            info!("🔀 Направление: {}", if a_to_b { "A→B" } else { "B→A" });

            // Рассчитываем ожидаемый выход
            let (estimated_out, min_out) =
                self.calculate_swap_amounts(pool, current_amount, a_to_b)?;

            info!("💸 Входная сумма: {} atoms", current_amount);
            info!("💰 Ожидаемый выход: {} atoms", estimated_out);
            info!("📉 Минимальный выход: {} atoms", min_out);

            let exchange_rate = if current_amount > 0 {
                estimated_out as f64 / current_amount as f64
            } else {
                0.0
            };
            info!("💹 Обменный курс: {:.6}", exchange_rate);

            if estimated_out > current_amount {
                info!(
                    "✅ Прибыльный своп (+{} atoms)",
                    estimated_out - current_amount
                );
            } else {
                info!(
                    "❌ Убыточный своп (-{} atoms)",
                    current_amount - estimated_out
                );
            }

            let leg = SwapLeg {
                protocol: pool.protocol,
                pool_id: pool.id,
                input_mint,
                output_mint,
                amount_in: current_amount,
                minimum_amount_out: min_out,
                estimated_amount_out: estimated_out,
                fee_bps: pool.fee_bps,
                pool_state_data: pool.full_state_data.clone(),
            };

            legs.push(leg);
            current_amount = estimated_out; // Для следующего свопа
        }

        // Расчёт чистой прибыли — работаем в атомах стартового токена
        let initial_amount = legs[0].amount_in;
        let final_amount = legs.last().unwrap().estimated_amount_out;

        info!("📊 === ИТОГОВЫЙ РАСЧЕТ ===");
        info!(
            "🏁 Начальная сумма: {} atoms (mint: {}, decimals: {})",
            initial_amount, start_mint, start_decimals
        );
        info!(
            "🎯 Финальная сумма: {} atoms (mint: {}, decimals: {})",
            final_amount, start_mint, start_decimals
        );

        // ProfitCalculator оставляем как есть — он работает на u64.
        // В devnet не учитываем SOL комиссии (они в другой единице).
        let (gross_profit, net_profit) =
            self.profit_calc
                .calculate_net_profit(initial_amount, final_amount, &legs)?;

        info!("💎 Валовая прибыль: {} atoms", gross_profit);
        info!("🏦 Чистая прибыль: {} atoms", net_profit);
        info!(
            "📊 Минимальный порог: {} atoms",
            self.config.trading.min_profit_lamports
        );

        // Проверка прибыльности
        if net_profit < self.config.trading.min_profit_lamports {
            info!(
                "❌ ОТКЛОНЕНО: Прибыль {} < {} (порог)",
                net_profit, self.config.trading.min_profit_lamports
            );
            info!("💡 Попробуйте понизить MIN_PROFIT_LAMPORTS в .env файле");
            return Ok(None);
        }

        let profit_percentage = if initial_amount > 0 {
            (net_profit as f64 / initial_amount as f64) * 100.0
        } else {
            0.0
        };

        info!("✅ ПРИНЯТО: Арбитражная возможность одобрена!");
        info!("📈 Процент прибыли: {:.4}%", profit_percentage);

        let opportunity = ArbitrageOpportunity {
            legs,
            initial_amount,
            expected_final_amount: final_amount,
            gross_profit,
            net_profit,
            profit_percentage,
            discovered_at: chrono::Utc::now().timestamp(),
        };

        Ok(Some(opportunity))
    }

    /// Расчёт ожидаемого и минимального выхода свопа в атомарных единицах токена
    fn calculate_swap_amounts(
        &self,
        pool: &PoolState,
        amount_in: u64,
        a_to_b: bool,
    ) -> Result<(u64, u64)> {
        let (reserve_in, reserve_out) = if a_to_b {
            (pool.reserve_a, pool.reserve_b)
        } else {
            (pool.reserve_b, pool.reserve_a)
        };

        // Для всех тестовых пулов (AMM/CPMM/DLMM) используем CPMM-формулу
        let estimated_out = {
            use crate::arbitrage::pool_math::calculate_cpmm_output;
            calculate_cpmm_output(reserve_in, reserve_out, amount_in, pool.fee_bps)?
        };

        // Минимальный выход с учётом slippage
        use crate::arbitrage::pool_math::calculate_minimum_amount_out;
        let min_out =
            calculate_minimum_amount_out(estimated_out, self.config.trading.max_slippage_bps);

        Ok((estimated_out, min_out))
    }
}