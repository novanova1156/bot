// bot/src/arbitrage/bellman_ford.rs

use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
// ИСПРАВЛЕНИЕ: Удаляем неиспользуемый HashMap
use std::collections::HashSet;
use tracing::{info, debug};

use super::graph::PriceGraph;

#[derive(Debug, Clone)]
pub struct ArbitrageCycle {
    pub tokens: Vec<Pubkey>,
    pub total_weight: f64,
}

pub struct BellmanFordSolver;

impl BellmanFordSolver {
    pub fn new() -> Self {
        Self
    }

    /// ИСПРАВЛЕННЫЙ поиск арбитражных циклов
    pub fn find_negative_cycles(
        &self,
        graph: &PriceGraph,
        _max_legs: usize, // ИСПРАВЛЕНИЕ: Добавили префикс _
    ) -> Result<Vec<ArbitrageCycle>> {
        let n = graph.token_count();

        info!("🔍 === ДИАГНОСТИКА ГРАФА ===");
        info!("Токенов в графе: {}", n);

        // Диагностика: показать все рёбра графа
        let mut total_edges = 0;
        for from in 0..n {
            for to in 0..n {
                let edges = graph.get_edges(from, to);
                if !edges.is_empty() {
                    total_edges += edges.len();
                    if let (Some(from_token), Some(to_token)) = (graph.get_token(from), graph.get_token(to)) {
                        info!("   Ребро: {}... -> {}... ({} вариантов)",
                             &from_token.to_string()[..8],
                             &to_token.to_string()[..8],
                             edges.len());
                    }
                }
            }
        }

        info!("Всего рёбер: {}", total_edges);

        if total_edges == 0 {
            info!("❌ ГРАФ ПУСТОЙ! Нет рёбер между токенами!");
            return Ok(vec![]);
        }

        // Ищем циклы методом прямого поиска
        let mut cycles = Vec::new();
        let mut found_cycles = HashSet::new();

        info!("🔄 === ПОИСК ТРЕУГОЛЬНЫХ АРБИТРАЖЕЙ ===");

        // Перебираем все возможные треугольники
        for start_idx in 0..n {
            for mid_idx in 0..n {
                if mid_idx == start_idx { continue; }

                for end_idx in 0..n {
                    if end_idx == start_idx || end_idx == mid_idx { continue; }

                    // Проверяем путь: start → mid → end → start
                    if let Some(cycle) = self.check_triangle_arbitrage(
                        graph, start_idx, mid_idx, end_idx
                    )? {
                        let cycle_signature = self.get_cycle_signature(&cycle);

                        if !found_cycles.contains(&cycle_signature) {
                            found_cycles.insert(cycle_signature);

                            info!("🎯 НАЙДЕН ТРЕУГОЛЬНЫЙ АРБИТРАЖ!");
                            info!("   Путь: {} → {} → {} → {}",
                                 &cycle.tokens[0].to_string()[..8],
                                 &cycle.tokens[1].to_string()[..8],
                                 &cycle.tokens[2].to_string()[..8],
                                 &cycle.tokens[3].to_string()[..8]);
                            info!("   Общий вес: {:.6}", cycle.total_weight);

                            if cycle.total_weight < -0.001 { // Прибыльный
                                info!("   ✅ ПРИБЫЛЬНЫЙ!");
                                cycles.push(cycle);
                            } else {
                                info!("   ❌ Не прибыльный");
                            }
                        }
                    }
                }
            }
        }

        info!("🏁 Найдено арбитражных циклов: {}", cycles.len());
        Ok(cycles)
    }

    /// Проверяем треугольный арбитраж A→B→C→A
    fn check_triangle_arbitrage(
        &self,
        graph: &PriceGraph,
        a_idx: usize,
        b_idx: usize,
        c_idx: usize,
    ) -> Result<Option<ArbitrageCycle>> {
        // Проверяем существование всех трёх рёбер
        let edges_ab = graph.get_edges(a_idx, b_idx);
        let edges_bc = graph.get_edges(b_idx, c_idx);
        let edges_ca = graph.get_edges(c_idx, a_idx);

        if edges_ab.is_empty() || edges_bc.is_empty() || edges_ca.is_empty() {
            return Ok(None); // Нет полного пути
        }

        // Берём первое доступное ребро для каждого перехода
        let edge_ab = &edges_ab[0];
        let edge_bc = &edges_bc[0];
        let edge_ca = &edges_ca[0];

        // Вычисляем общий вес цикла
        let total_weight = edge_ab.weight + edge_bc.weight + edge_ca.weight;

        debug!("   Проверка цикла {}->{}->{} = {:.6}",
               a_idx, b_idx, c_idx, total_weight);

        let tokens = vec![
            edge_ab.from_token,
            edge_ab.to_token,
            edge_bc.to_token,
            edge_ca.to_token, // Возврат к началу
        ];

        Ok(Some(ArbitrageCycle {
            tokens,
            total_weight,
        }))
    }

    /// Получение подписи цикла для дедупликации
    fn get_cycle_signature(&self, cycle: &ArbitrageCycle) -> String {
        let mut tokens_str: Vec<String> = cycle.tokens[..cycle.tokens.len()-1]
            .iter()
            .map(|t| t.to_string())
            .collect();

        tokens_str.sort(); // Сортируем для нормализации
        tokens_str.join("-")
    }
}