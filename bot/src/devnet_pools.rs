// bot/src/devnet_pools.rs (НОВАЯ ВЕРСИЯ ДЛЯ АРБИТРАЖА CLMM)
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use anyhow::Result;
use crate::types::{DexProtocol, PoolState};
use chrono::Utc;
pub fn get_devnet_pools() -> Result<Vec<PoolState>> {
    let mut pools = Vec::new();
    // Ваши токены
    let token_a = Pubkey::from_str("7oa4krfxjocDH47RymzbPW4QHVV4Ec4vuAQj1gYAn3SQ")?; // TOKEN_A
    let token_b = Pubkey::from_str("4PvUes3azNmTSohsrSBBDTFZqVQqM5oCkdF1vZDpPoZS")?; // TOKEN_B
    let token_c = Pubkey::from_str("ExAMh7G7BRG5qVLFJySeedkCPh39ywpVBNFMAM5rmpdc")?; // TOKEN_C
    // SOL_MINT теперь не используется в цикле, но сохраним его для справки
    // let sol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112")?;

    // Единица измерения для резервов
    let unit: u64 = 1_000_000_000_000;

    // Пул #1: TOKEN_A - TOKEN_B (CLMM pool) - 1:1
    pools.push(PoolState {
        id: Pubkey::from_str("J5PfV8u3EvXLRBsKQdpEiuwNVeQffDbCPP5zisRikGu8")?,
        protocol: DexProtocol::RaydiumClmm,
        token_a,
        token_b,
        reserve_a: unit,
        reserve_b: unit,
        fee_bps: 30,
        last_updated: Utc::now().timestamp(),
        full_state_data: vec![],
        decimals_a: 9,
        decimals_b: 9,
    });

    // Пул #2: TOKEN_B - TOKEN_C (CLMM pool) - 1:1
    pools.push(PoolState {
        id: Pubkey::from_str("DMJ5DmzQLiSRNLuSR7HdwCz8KDdgywtMJZKSYN5EPbBk")?,
        protocol: DexProtocol::RaydiumClmm,
        token_a: token_b,
        token_b: token_c,
        reserve_a: unit,
        reserve_b: unit,
        fee_bps: 30,
        last_updated: Utc::now().timestamp(),
        full_state_data: vec![],
        decimals_a: 9,
        decimals_b: 9,
    });

    // Пул #3: TOKEN_C - TOKEN_A (CLMM pool) - ДИСБАЛАНС 10:1 (Для создания возможности)
    // Соотношение: 1 TOKEN_A = 10 TOKEN_C
    pools.push(PoolState {
        id: Pubkey::from_str("4289ioWPRqHeprQJAyZnRTy7p1P4L82ePg9PQpjG4p8K")?,
        protocol: DexProtocol::RaydiumClmm,
        token_a: token_c, // TOKEN_C (Резерв 1)
        token_b: token_a, // TOKEN_A (Резерв 10)
        reserve_a: unit,      // 1000 C
        reserve_b: unit * 10, // 10000 A
        fee_bps: 30,
        last_updated: Utc::now().timestamp(),
        full_state_data: vec![],
        decimals_a: 9,
        decimals_b: 9,
    });

    // Теперь у нас есть A-B, B-C, C-A. Это замкнутый цикл.

    Ok(pools)
}