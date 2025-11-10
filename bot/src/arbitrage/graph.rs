// bot/src/arbitrage/graph.rs
// Построение графа цен для поиска арбитража

use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use tracing::debug;

use crate::types::{PoolState, PriceEdge};

/// Граф цен между токенами
pub struct PriceGraph {
    /// Соответствие токен -> индекс в графе
    token_to_index: HashMap<Pubkey, usize>,
    /// Соответствие индекс -> токен
    index_to_token: Vec<Pubkey>,
    /// Матрица смежности: adjacency[from][to] = vec![PriceEdge]
    adjacency: Vec<Vec<Vec<PriceEdge>>>,
}

impl PriceGraph {
    pub fn new() -> Self {
        Self {
            token_to_index: HashMap::new(),
            index_to_token: Vec::new(),
            adjacency: Vec::new(),
        }
    }

    /// Построение графа из списка пулов
    pub fn build_from_pools(&self, pools: &[PoolState]) -> Result<PriceGraph> {
        let mut graph = PriceGraph::new();

        // Шаг 1: Собираем все уникальные токены
        for pool in pools {
            graph.add_token_if_new(pool.token_a);
            graph.add_token_if_new(pool.token_b);
        }
        let max_pools = 2000;
        let pools = if pools.len() > max_pools {
            tracing::warn!("⚠️ Ограничение пулов с {} до {}", pools.len(), max_pools);
            &pools[..max_pools]
        } else {
            pools
        };

        // Собираем токены и считаем, сколько будет узлов
        // ...
        let n = graph.token_count();

        // Если токенов слишком много — останавливаемся заранее
        let max_tokens = if pools.len() < 100 { 1000 } else { 100 }; // ДИНАМИЧЕСКИЙ ЛИМИТ
        if n > max_tokens {
            anyhow::bail!("Слишком большой граф: {} токенов (> {})", n, max_tokens);
        }
        // Инициализация матрицы смежности
        let n = graph.token_count();
        graph.adjacency = vec![vec![Vec::new(); n]; n];

        // Шаг 2: Добавляем рёбра для каждого пула
        for pool in pools {
            // Направление A -> B
            let edge_ab = PriceEdge {
                from_token: pool.token_a,
                to_token: pool.token_b,
                pool_id: pool.id,
                protocol: pool.protocol,
                weight: Self::calculate_edge_weight(pool, true)?,
                fee_bps: pool.fee_bps,
            };

            // Направление B -> A
            let edge_ba = PriceEdge {
                from_token: pool.token_b,
                to_token: pool.token_a,
                pool_id: pool.id,
                protocol: pool.protocol,
                weight: Self::calculate_edge_weight(pool, false)?,
                fee_bps: pool.fee_bps,
            };

            graph.add_edge(edge_ab)?;
            graph.add_edge(edge_ba)?;
        }

        debug!("Граф построен: {} токенов, {} рёбер", n, graph.edge_count());
        Ok(graph)
    }

    /// Вычисление веса ребра (отрицательный логарифм обменного курса)
    /// Bellman-Ford находит минимальные пути, отрицательный цикл = прибыль
    fn calculate_edge_weight(pool: &PoolState, a_to_b: bool) -> Result<f64> {
        let (reserve_in, reserve_out) = if a_to_b {
            (pool.reserve_a as f64, pool.reserve_b as f64)
        } else {
            (pool.reserve_b as f64, pool.reserve_a as f64)
        };

        if reserve_in <= 0.0 || reserve_out <= 0.0 {
            anyhow::bail!("Нулевые резервы в пуле");
        }

        // Учёт комиссии (fee_bps / 10000)
        let fee_multiplier = 1.0 - (pool.fee_bps as f64 / 10000.0);

        // Обменный курс с учётом комиссии
        let exchange_rate = (reserve_out / reserve_in) * fee_multiplier;

        // Вес = -log(exchange_rate)
        // Отрицательный цикл означает произведение курсов > 1 (прибыль)
        let weight = -(exchange_rate.ln());

        Ok(weight)
    }

    /// Добавление токена если ещё не существует
    fn add_token_if_new(&mut self, token: Pubkey) {
        if !self.token_to_index.contains_key(&token) {
            let index = self.index_to_token.len();
            self.token_to_index.insert(token, index);
            self.index_to_token.push(token);
        }
    }

    /// Добавление ребра в граф
    fn add_edge(&mut self, edge: PriceEdge) -> Result<()> {
        let from_idx = self.token_to_index.get(&edge.from_token)
            .ok_or_else(|| anyhow::anyhow!("Токен не найден в графе"))?;
        let to_idx = self.token_to_index.get(&edge.to_token)
            .ok_or_else(|| anyhow::anyhow!("Токен не найден в графе"))?;

        self.adjacency[*from_idx][*to_idx].push(edge);
        Ok(())
    }

    /// Получение токена по индексу
    pub fn get_token(&self, index: usize) -> Option<&Pubkey> {
        self.index_to_token.get(index)
    }

    /// Получение индекса токена
    pub fn get_index(&self, token: &Pubkey) -> Option<usize> {
        self.token_to_index.get(token).copied()
    }

    /// Получение всех рёбер между двумя токенами
    pub fn get_edges(&self, from: usize, to: usize) -> &[PriceEdge] {
        &self.adjacency[from][to]
    }

    /// Количество токенов в графе
    pub fn token_count(&self) -> usize {
        self.index_to_token.len()
    }

    /// Количество рёбер в графе
    pub fn edge_count(&self) -> usize {
        self.adjacency.iter()
            .flat_map(|row| row.iter())
            .map(|edges| edges.len())
            .sum()
    }
}