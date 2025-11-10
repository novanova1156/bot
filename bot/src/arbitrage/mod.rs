// bot/src/arbitrage/mod.rs
// Оркестратор поиска арбитражных возможностей

pub mod graph;
pub mod bellman_ford;
pub mod opportunity;
pub mod profit_calculator;
pub mod pool_math;

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, debug};

use crate::config::BotConfig;
use crate::types::{PoolState, ArbitrageOpportunity};
use graph::PriceGraph;
use bellman_ford::BellmanFordSolver;
use opportunity::OpportunityEvaluator;

pub struct ArbitrageFinder {
    config: Arc<BotConfig>,
    graph_builder: PriceGraph,
    solver: BellmanFordSolver,
    evaluator: OpportunityEvaluator,
}

impl ArbitrageFinder {
    pub fn new(config: Arc<BotConfig>) -> Self {
        Self {
            config: config.clone(),
            graph_builder: PriceGraph::new(),
            solver: BellmanFordSolver::new(),
            evaluator: OpportunityEvaluator::new(config),
        }
    }

    /// Поиск всех арбитражных возможностей в заданных пулах
    pub fn find_opportunities(&self, pools: &[PoolState]) -> Result<Vec<ArbitrageOpportunity>> {
        if pools.is_empty() {
            return Ok(vec![]);
        }

        info!("🔍 Построение графа цен из {} пулов...", pools.len());

        // Шаг 1: Построение графа цен
        let graph = self.graph_builder.build_from_pools(pools)?;
        debug!("   Граф содержит {} токенов, {} рёбер",
           graph.token_count(),
           graph.edge_count());

        // Шаг 2: Поиск отрицательных циклов через Bellman-Ford
        info!("🧮 Применение алгоритма Bellman-Ford для поиска циклов...");
        let cycles = self.solver.find_negative_cycles(&graph, self.config.trading.max_legs as usize)?;

        if cycles.is_empty() {
            debug!("   Отрицательных циклов не найдено");
            return Ok(vec![]);
        }

        info!("   Найдено потенциальных циклов: {}", cycles.len());

        // ДИАГНОСТИКА: Показать информацию о каждом цикле
        for (i, cycle) in cycles.iter().enumerate() {
            info!("🔄 Цикл #{}: {} токенов, вес {:.6}",
             i + 1, cycle.tokens.len(), cycle.total_weight);
            info!("   Токены: {:?}", cycle.tokens.iter()
             .map(|t| format!("{}...", &t.to_string()[..8]))
             .collect::<Vec<_>>());
        }

        // Шаг 3: Оценка прибыльности каждого цикла
        let mut opportunities = Vec::new();

        for (i, cycle) in cycles.iter().enumerate() {
            info!("🧮 === АНАЛИЗ ЦИКЛА #{} ===", i + 1);

            match self.evaluator.evaluate_cycle(cycle, pools) {
                Ok(Some(opp)) => {
                    info!("✅ Цикл #{} ПРИБЫЛЕН!", i + 1);
                    // Проверка минимальной прибыли
                    if opp.net_profit >= self.config.trading.min_profit_lamports {
                        opportunities.push(opp);
                    }
                }
                Ok(None) => {
                    info!("❌ Цикл #{} отклонен", i + 1);
                }
                Err(e) => {
                    info!("⚠️ Ошибка анализа цикла #{}: {}", i + 1, e);
                }
            }
        }

        // Сортировка по убыванию прибыли
        opportunities.sort_by(|a, b| b.net_profit.cmp(&a.net_profit));

        info!("✅ Найдено прибыльных возможностей: {}", opportunities.len());

        Ok(opportunities)
    }
}