// programs/arbitrage-executor/src/lib.rs
// Smart Contract "Executor" for Atomic Multi-Hop Swaps via CPI
#![allow(deprecated)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{invoke},
    pubkey::Pubkey,
    msg,

};
use anchor_spl::token::{Token, TokenAccount, Mint};
use anchor_lang::solana_program::pubkey;

declare_id!("HXccYBQu47LExrec1CAUBybYsXQL2pkEEdTaSD9emRY9");

// ============================================================================
// DEX PROGRAM IDS (MAINNET)
// ============================================================================

pub const RAYDIUM_AMM_V4: Pubkey = pubkey!("DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav");
pub const RAYDIUM_CPMM: Pubkey = pubkey!("DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb");
pub const RAYDIUM_CLMM: Pubkey = pubkey!("DRayAUgENGQBKVaX8owNhgzkEDyoHTGVEGHVJT1E9pfH");
pub const METEORA_DLMM: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

// ============================================================================
// TYPES & ENUMS
// ============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DexProtocol {
    RaydiumAmmV4,
    RaydiumCpmm,
    RaydiumClmm,
    MeteoraDlmm,
}

impl DexProtocol {
    pub fn program_id(&self) -> Pubkey {
        match self {
            DexProtocol::RaydiumAmmV4 => RAYDIUM_AMM_V4,
            DexProtocol::RaydiumCpmm => RAYDIUM_CPMM,
            DexProtocol::RaydiumClmm => RAYDIUM_CLMM,
            DexProtocol::MeteoraDlmm => METEORA_DLMM,
        }
    }
}

/// Single swap leg in multi-hop route
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SwapLeg {
    pub protocol: DexProtocol,
    pub pool_id: Pubkey,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub amount_in: u64,
    pub minimum_amount_out: u64,
    /// Number of accounts needed for CPI (extracted from remaining_accounts)
    pub accounts_len: u8,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum ArbitrageError {
    #[msg("Insufficient profit - arbitrage not profitable")]
    InsufficientProfit,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Invalid number of accounts provided")]
    InvalidAccountsCount,
    #[msg("Invalid DEX protocol specified")]
    InvalidDexProtocol,
    #[msg("Math overflow during calculation")]
    MathOverflow,
    #[msg("Unauthorized user")]
    Unauthorized,
    #[msg("Too many swap legs (max 5)")]
    TooManyLegs,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("CPI call failed")]
    CpiCallFailed,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
}

// ============================================================================
// PROGRAM MODULE
// ============================================================================

#[program]
pub mod arbitrage_executor {
    use super::*;

    /// Execute atomic multi-hop arbitrage swap
    ///
    /// # Parameters
    /// - `swap_legs`: Sequence of swaps to execute
    /// - `min_profit_lamports`: Minimum required profit in lamports
    ///
    /// # Logic
    /// 1. Record initial balance
    /// 2. Execute each swap via CPI to respective DEX
    /// 3. Verify final balance >= initial + min_profit
    /// 4. If profit insufficient - transaction reverts
    pub fn execute_arbitrage<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, ExecuteArbitrage<'info>>,
        swap_legs: Vec<SwapLeg>,
        min_profit_lamports: u64,
    ) -> Result<()>
    where
        'c: 'info,
    {
        // Validate swap legs count (max 5 to limit compute units)
        require!(
            !swap_legs.is_empty() && swap_legs.len() <= 5,
            ArbitrageError::TooManyLegs
        );

        msg!(
            "🚀 Starting arbitrage: {} legs, min profit {} lamports",
            swap_legs.len(),
            min_profit_lamports
        );

        // Record initial balance
        let initial_balance = ctx.accounts.user_token_account.amount;
        msg!("💰 Initial balance: {} lamports", initial_balance);

        // Validate sufficient balance
        require!(
            initial_balance >= swap_legs[0].amount_in,
            ArbitrageError::InsufficientBalance
        );

        // Execute each swap leg
        let mut account_cursor = 0_usize;

        for (idx, leg) in swap_legs.iter().enumerate() {
            msg!(
                "📊 Leg {}/{}: {:?} on pool {}",
                idx + 1,
                swap_legs.len(),
                leg.protocol,
                leg.pool_id
            );

            // Extract accounts for current leg from remaining_accounts
            let accounts_end = account_cursor
                .checked_add(leg.accounts_len as usize)
                .ok_or(ArbitrageError::MathOverflow)?;
            require!(
            accounts_end <= ctx.remaining_accounts.len(),
            ArbitrageError::InvalidAccountsCount
        );

            let leg_accounts = &ctx.remaining_accounts[account_cursor..accounts_end];

            // Execute swap via CPI
            execute_swap_cpi(leg, leg_accounts, &ctx.accounts.user)?;

            account_cursor = accounts_end;

            // Reload balance after intermediate step
            if idx < swap_legs.len() - 1 {
                ctx.accounts.user_token_account.reload()?;
                msg!("   Intermediate balance: {}", ctx.accounts.user_token_account.amount);
            }
        }

        // Final profitability check
        ctx.accounts.user_token_account.reload()?;
        let final_balance = ctx.accounts.user_token_account.amount;

        msg!("💎 Final balance: {} lamports", final_balance);

        // Calculate net profit
        let profit = final_balance
            .checked_sub(initial_balance)
            .ok_or(ArbitrageError::MathOverflow)?;

        msg!(
            "📈 Profit: {} lamports ({:.4}%)",
            profit,
            (profit as f64 / initial_balance as f64) * 100.0
        );

        // Verify minimum profit (CRITICAL: reverts if insufficient)
        require!(
            profit >= min_profit_lamports,
            ArbitrageError::InsufficientProfit
        );

        msg!("✅ ARBITRAGE SUCCESSFUL");

        // Emit event for monitoring
        emit!(ArbitrageExecutedEvent {
            user: ctx.accounts.user.key(),
            initial_balance,
            final_balance,
            profit,
            legs_count: swap_legs.len() as u8,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}

// ============================================================================
// ACCOUNT STRUCTURES
// ============================================================================

#[derive(Accounts)]
pub struct ExecuteArbitrage<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ ArbitrageError::Unauthorized
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        constraint = user_token_account.mint == token_mint.key() @ ArbitrageError::InvalidTokenAccount
    )]
    pub token_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct ArbitrageExecutedEvent {
    pub user: Pubkey,
    pub initial_balance: u64,
    pub final_balance: u64,
    pub profit: u64,
    pub legs_count: u8,
    pub timestamp: i64,
}

// ============================================================================
// CPI HELPER FUNCTIONS
// ============================================================================

/// Execute swap via CPI to DEX
fn execute_swap_cpi<'info>(
    leg: &SwapLeg,
    accounts: &'info [AccountInfo<'info>],
    user: &Signer<'info>,
) -> Result<()> {
    match leg.protocol {
        DexProtocol::RaydiumAmmV4 => raydium_amm_swap(leg, accounts, user),
        DexProtocol::RaydiumCpmm => raydium_cpmm_swap(leg, accounts, user),
        DexProtocol::RaydiumClmm => raydium_clmm_swap(leg, accounts, user),
        DexProtocol::MeteoraDlmm => meteora_dlmm_swap(leg, accounts, user), // Ошибка E0425 здесь
    }
}

/// Raydium AMM V4 swap_base_in CPI
fn raydium_amm_swap<'info>(
    leg: &SwapLeg,
    accounts: &'info [AccountInfo<'info>],
    _user: &Signer<'info>,
) -> Result<()> {
    require!(accounts.len() == 18, ArbitrageError::InvalidAccountsCount);

    // Raydium AMM V4 swap_base_in discriminator: 0x09
    let mut data = vec![0x09];
    data.extend_from_slice(&leg.amount_in.to_le_bytes());
    data.extend_from_slice(&leg.minimum_amount_out.to_le_bytes());

    let account_metas: Vec<AccountMeta> = accounts
        .iter()
        .map(|acc| AccountMeta {
            pubkey: *acc.key,
            is_signer: acc.is_signer,
            is_writable: acc.is_writable,
        })
        .collect();

    let ix = Instruction {
        program_id: RAYDIUM_AMM_V4,
        accounts: account_metas,
        data,
    };

    invoke(&ix, accounts).map_err(|_| error!(ArbitrageError::CpiCallFailed))?;

    msg!("   ✅ Raydium AMM swap executed");
    Ok(())
}

/// Raydium CPMM swap CPI
fn raydium_cpmm_swap<'info>(
    leg: &SwapLeg,
    accounts: &'info [AccountInfo<'info>],
    _user: &Signer<'info>,
) -> Result<()> {
    require!(accounts.len() >= 10, ArbitrageError::InvalidAccountsCount);

    // FIX: Используем 1-байтовый Instruction ID Raydium CPMM Swap (0x01)
    // Вместо 8-байтового Anchor-дискриминатора.
    let mut data = vec![0x01];
    data.extend_from_slice(&leg.amount_in.to_le_bytes());
    data.extend_from_slice(&leg.minimum_amount_out.to_le_bytes());

    let account_metas: Vec<AccountMeta> = accounts
        .iter()
        .map(|acc| AccountMeta {
            pubkey: *acc.key,
            is_signer: acc.is_signer,
            is_writable: acc.is_writable,
        })
        .collect();

    let ix = Instruction {
        program_id: RAYDIUM_CPMM,
        accounts: account_metas,
        data,
    };

    invoke(&ix, accounts).map_err(|_| error!(ArbitrageError::CpiCallFailed))?;

    msg!("   ✅ Raydium CPMM swap executed");
    Ok(())
}

/// Raydium CLMM swap CPI
fn raydium_clmm_swap<'info>(
    leg: &SwapLeg,
    accounts: &'info [AccountInfo<'info>],
    _user: &Signer<'info>,
) -> Result<()> {
    require!(accounts.len() >= 13, ArbitrageError::InvalidAccountsCount);

    // ПРАВИЛЬНЫЙ дискриминатор для swap_v2 (sha256("global:swap_v2")[..8])
    // УДАЛЕНЫ ЛИШНИЕ СИМВОЛЫ **
    let mut data: Vec<u8> = vec![0xf3, 0x0c, 0x03, 0x33, 0x8f, 0x93, 0x18, 0x39];

    // Параметры для swap_v2
    data.extend_from_slice(&leg.amount_in.to_le_bytes());          // amount: u64
    data.extend_from_slice(&leg.minimum_amount_out.to_le_bytes());  // other_amount_threshold: u64
    data.extend_from_slice(&(0_u128).to_le_bytes());              // sqrt_price_limit_x64: u128 (0 = no limit)
    // data.extend_from_slice(&(1_u8).to_le_bytes());                // is_base_input: bool (true)

    // УДАЛЕНЫ ЛИШНИЕ СИМВОЛЫ **
    let account_metas: Vec<AccountMeta> = accounts
        .iter()
        .map(|acc| AccountMeta {
            pubkey: *acc.key,
            is_signer: acc.is_signer,
            is_writable: acc.is_writable,
        })
        .collect();

    let ix = Instruction {
        program_id: RAYDIUM_CLMM,
        accounts: account_metas,
        data,
    };

    invoke(&ix, accounts).map_err(|_| error!(ArbitrageError::CpiCallFailed))?;

    msg!("   ✅ Raydium CLMM swap_v2 executed");
    Ok(())
}

// **(3) ДОБАВЛЕНА ОТСУТСТВУЮЩАЯ ФУНКЦИЯ meteora_dlmm_swap (ИСПРАВЛЕНИЕ E0425)**
/// Meteora DLMM swap CPI
fn meteora_dlmm_swap<'info>(
    leg: &SwapLeg,
    accounts: &'info [AccountInfo<'info>],
    _user: &Signer<'info>,
) -> Result<()> {
    require!(accounts.len() >= 10, ArbitrageError::InvalidAccountsCount);

    // Meteora DLMM swap discriminator
    let mut data = vec![0x13, 0x98, 0xa2, 0x5f, 0x5e, 0x8f, 0x2d, 0x7c];
    data.extend_from_slice(&leg.amount_in.to_le_bytes());
    data.extend_from_slice(&leg.minimum_amount_out.to_le_bytes());

    let account_metas: Vec<AccountMeta> = accounts
        .iter()
        .map(|acc| AccountMeta {
            pubkey: *acc.key,
            is_signer: acc.is_signer,
            is_writable: acc.is_writable,
        })
        .collect();

    let ix = Instruction {
        program_id: METEORA_DLMM,
        accounts: account_metas,
        data,
    };

    invoke(&ix, accounts).map_err(|_| error!(ArbitrageError::CpiCallFailed))?;

    msg!("   ✅ Meteora DLMM swap executed");
    Ok(())
}