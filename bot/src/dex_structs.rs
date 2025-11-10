// bot/src/dex_structs.rs
use solana_sdk::pubkey::Pubkey;
use std::convert::TryInto;
use anyhow::{Result, anyhow};

/// Минимальная длина данных для Raydium AMM V4 AmmInfo
const MIN_AMM_INFO_LEN: usize = 752;

// ПРАВИЛЬНЫЕ ОФСЕТЫ ДЛЯ RAYDIUM AMM V4
const MARKET_ID_OFFSET: usize = 224;
const MARKET_PROGRAM_ID_OFFSET: usize = 256;
const BASE_VAULT_OFFSET: usize = 384;
const QUOTE_VAULT_OFFSET: usize = 416;
const BASE_MINT_OFFSET: usize = 632;
const QUOTE_MINT_OFFSET: usize = 664;

// Дополнительные важные поля
const STATUS_OFFSET: usize = 0;
const NONCE_OFFSET: usize = 8;
const OPEN_ORDERS_OFFSET: usize = 168;
const TARGET_ORDERS_OFFSET: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct AmmInfo {
    pub status: u64,
    pub nonce: u64,
    pub market_id: Pubkey,
    pub market_program_id: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub open_orders: Pubkey,
    pub target_orders: Pubkey,
    pub fees: Fees,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fees {
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
}

impl AmmInfo {
    /// Парсинг AmmInfo из сырых данных аккаунта с правильными офсетами
    pub fn try_from_slice(data: &[u8]) -> Result<Self> {
        if data.len() < MIN_AMM_INFO_LEN {
            return Err(anyhow!(
                "Недостаточно данных для AmmInfo: {} байт, требуется минимум {}",
                data.len(),
                MIN_AMM_INFO_LEN
            ));
        }

        let status = read_u64(data, STATUS_OFFSET)?;
        let nonce = read_u64(data, NONCE_OFFSET)?;
        let market_id = read_pubkey(data, MARKET_ID_OFFSET)?;
        let market_program_id = read_pubkey(data, MARKET_PROGRAM_ID_OFFSET)?;
        let base_mint = read_pubkey(data, BASE_MINT_OFFSET)?;
        let quote_mint = read_pubkey(data, QUOTE_MINT_OFFSET)?;
        let base_vault = read_pubkey(data, BASE_VAULT_OFFSET)?;
        let quote_vault = read_pubkey(data, QUOTE_VAULT_OFFSET)?;
        let open_orders = read_pubkey(data, OPEN_ORDERS_OFFSET)?;
        let target_orders = read_pubkey(data, TARGET_ORDERS_OFFSET)?;

        // Упрощенные комиссии (реальные офсеты зависят от версии)
        let fees = Fees {
            swap_fee_numerator: 25,     // 0.25% = 25 bps
            swap_fee_denominator: 10_000,
        };

        Ok(Self {
            status,
            nonce,
            market_id,
            market_program_id,
            base_mint,
            quote_mint,
            base_vault,
            quote_vault,
            open_orders,
            target_orders,
            fees,
        })
    }
}

// -------------------------------------------------------------------------
// RAYDIUM CPMM (Constant Product Market Maker)
// -------------------------------------------------------------------------

/// Минимальная длина данных для Raydium CPMM Pool
const MIN_CPMM_INFO_LEN: usize = 112;

// ОФСЕТЫ ДЛЯ RAYDIUM CPMM (ТИПИЧНЫЕ)
const CPMM_AUTHORITY_OFFSET: usize = 16;
const CPMM_VAULT_A_OFFSET: usize = 48;
const CPMM_VAULT_B_OFFSET: usize = 80;
const CPMM_MINT_A_OFFSET: usize = 112;
const CPMM_MINT_B_OFFSET: usize = 144;

#[derive(Debug, Clone, PartialEq)]
pub struct CpmmPoolInfo {
    pub status: u64,
    pub authority: Pubkey,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
}

impl CpmmPoolInfo {
    /// Парсинг CpmmPoolInfo из сырых данных аккаунта
    pub fn try_from_slice(data: &[u8]) -> Result<Self> {
        if data.len() < MIN_CPMM_INFO_LEN {
            return Err(anyhow!(
                "Недостаточно данных для CpmmPoolInfo: {} байт, требуется минимум {}",
                data.len(),
                MIN_CPMM_INFO_LEN
            ));
        }

        let status = 1; // Заглушка, так как статус не всегда на офсете 0
        let authority = read_pubkey(data, CPMM_AUTHORITY_OFFSET)?;
        let vault_a = read_pubkey(data, CPMM_VAULT_A_OFFSET)?;
        let vault_b = read_pubkey(data, CPMM_VAULT_B_OFFSET)?;

        // !!! ПРОВЕРЬТЕ эти офсеты в структуре Raydium CPMM !!!
        let mint_a = read_pubkey(data, CPMM_MINT_A_OFFSET).unwrap_or(Pubkey::new_unique());
        let mint_b = read_pubkey(data, CPMM_MINT_B_OFFSET).unwrap_or(Pubkey::new_unique());

        Ok(Self {
            status,
            authority,
            vault_a,
            vault_b,
            mint_a,
            mint_b,
        })
    }
}

// -------------------------------------------------------------------------
// RAYDIUM CLMM (Concentrated Liquidity Market Maker)
// -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ClmmPoolInfo {
    pub authority: Pubkey,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub mint_a: Pubkey, // ДОБАВЛЕНО
    pub mint_b: Pubkey, // ДОБАВЛЕНО
}

impl ClmmPoolInfo {
    // ВНИМАНИЕ: Офсеты CLMM очень специфичны. Требуется верификация!
    pub fn try_from_slice(data: &[u8]) -> Result<Self> {
        if data.len() < 104 { // Минимальный размер для 3 Pubkey + 8 байт (дискриминатор)
            return Err(anyhow!("Недостаточно данных для ClmmPoolInfo: требуется минимум 104 байт"));
        }

        // Фактические офсеты CLMM (Authority = 8, VaultA = 40, VaultB = 72)
        let authority = read_pubkey(data, 8)?;
        let vault_a = read_pubkey(data, 40)?;
        let vault_b = read_pubkey(data, 72)?;

        // ПРИМЕРНЫЕ ОФСЕТЫ ДЛЯ MINT (Требуется верификация!)
        let mint_a = read_pubkey(data, 104).unwrap_or(Pubkey::new_unique());
        let mint_b = read_pubkey(data, 136).unwrap_or(Pubkey::new_unique());


        Ok(Self {
            authority,
            vault_a,
            vault_b,
            mint_a,
            mint_b,
        })
    }
}


/// Чтение Pubkey из данных по офсету с проверкой границ
pub fn read_pubkey(data: &[u8], offset: usize) -> Result<Pubkey> {
    if offset + 32 > data.len() {
        return Err(anyhow!(
            "Недостаточно данных для Pubkey по офсету {}: нужно еще {} байт, есть {}",
            offset,
            32,
            data.len().saturating_sub(offset)
        ));
    }

    let slice = &data[offset..offset + 32];
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(slice);

    Ok(Pubkey::new_from_array(bytes))
}

/// Чтение u64 (little-endian) из данных по офсету
pub fn read_u64(data: &[u8], offset: usize) -> Result<u64> {
    if offset + 8 > data.len() {
        return Err(anyhow!(
            "Недостаточно данных для u64 по офсету {}: нужно еще {} байт, есть {}",
            offset,
            8,
            data.len().saturating_sub(offset)
        ));
    }

    let slice = &data[offset..offset + 8];
    let bytes: [u8; 8] = slice.try_into()?;
    Ok(u64::from_le_bytes(bytes))
}