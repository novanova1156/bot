// bot/src/types.rs
// Shared types and structures

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DexProtocol {
    RaydiumAmmV4,
    RaydiumCpmm,
    RaydiumClmm,
    MeteoraDlmm,
}

impl fmt::Display for DexProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DexProtocol::RaydiumAmmV4 => write!(f, "Raydium AMM V4"),
            DexProtocol::RaydiumCpmm => write!(f, "Raydium CPMM"),
            DexProtocol::RaydiumClmm => write!(f, "Raydium CLMM"),
            DexProtocol::MeteoraDlmm => write!(f, "Meteora DLMM"),
        }
    }
}

/// Pool state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    pub id: Pubkey,
    pub protocol: DexProtocol,
    pub token_a: Pubkey,
    pub token_b: Pubkey,
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub fee_bps: u16,
    pub last_updated: i64,
    pub full_state_data: Vec<u8>,
    pub decimals_a: u8,
    pub decimals_b: u8,
}

impl PoolState {
    pub fn price_a_to_b(&self) -> f64 {
        if self.reserve_b == 0 {
            return 0.0;
        }
        self.reserve_a as f64 / self.reserve_b as f64
    }

    pub fn price_b_to_a(&self) -> f64 {
        if self.reserve_a == 0 {
            return 0.0;
        }
        self.reserve_b as f64 / self.reserve_a as f64
    }
}

/// Single swap leg in arbitrage route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapLeg {
    pub protocol: DexProtocol,
    pub pool_id: Pubkey,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub amount_in: u64,
    pub minimum_amount_out: u64,
    pub estimated_amount_out: u64,
    pub fee_bps: u16,
    pub pool_state_data: Vec<u8>,
}

/// Complete arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub legs: Vec<SwapLeg>,
    pub initial_amount: u64,
    pub expected_final_amount: u64,
    pub gross_profit: u64,
    pub net_profit: u64,
    pub profit_percentage: f64,
    pub discovered_at: i64,
}

impl ArbitrageOpportunity {
    pub fn is_profitable(&self, min_profit: u64) -> bool {
        self.net_profit >= min_profit
    }
}

/// Price edge in graph
#[derive(Debug, Clone, Copy)]
pub struct PriceEdge {
    pub from_token: Pubkey,
    pub to_token: Pubkey,
    pub pool_id: Pubkey,
    pub protocol: DexProtocol,
    pub weight: f64, // -log(exchange_rate) for Bellman-Ford
    pub fee_bps: u16,
}

pub struct SimulationResult {
    pub err: Option<String>,
    pub logs: Vec<String>,
    pub units_consumed: Option<u64>,
}