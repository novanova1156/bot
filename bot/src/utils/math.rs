// bot/src/utils/math.rs
// Математические утилиты для расчётов арбитража

use anyhow::Result;

/// Расчёт выхода свопа по формуле постоянного произведения (AMM)
///
/// # Формула
/// amount_out = (reserve_out * amount_in) / (reserve_in + amount_in)
///
/// # Параметры
/// - reserve_in: Резерв входного токена
/// - reserve_out: Резерв выходного токена
/// - amount_in: Количество входного токена
/// - fee_bps: Комиссия в базисных пунктах (25 = 0.25%)
pub fn calculate_amm_swap_output(
    reserve_in: u64,
    reserve_out: u64,
    amount_in: u64,
    fee_bps: u16,
) -> Result<u64> {
    if reserve_in == 0 || reserve_out == 0 {
        anyhow::bail!("Нулевые резервы в пуле");
    }

    // Применение комиссии к входной сумме
    let fee_multiplier = 1.0 - (fee_bps as f64 / 10000.0);
    let amount_in_with_fee = (amount_in as f64) * fee_multiplier;

    // Формула постоянного произведения
    let numerator = (reserve_out as f64) * amount_in_with_fee;
    let denominator = (reserve_in as f64) + amount_in_with_fee;

    let amount_out = numerator / denominator;

    Ok(amount_out as u64)
}

/// Расчёт минимального выхода с учётом slippage
///
/// # Параметры
/// - expected_amount: Ожидаемая сумма
/// - slippage_bps: Допустимый slippage в базисных пунктах (100 = 1%)
pub fn calculate_minimum_amount_out(
    expected_amount: u64,
    slippage_bps: u16,
) -> u64 {
    let slippage_multiplier = 1.0 - (slippage_bps as f64 / 10000.0);
    ((expected_amount as f64) * slippage_multiplier) as u64
}

/// Расчёт цены токена A в терминах токена B
pub fn calculate_price_a_to_b(reserve_a: u64, reserve_b: u64) -> f64 {
    if reserve_b == 0 {
        return 0.0;
    }
    reserve_a as f64 / reserve_b as f64
}

/// Расчёт изменения цены в процентах
pub fn calculate_price_change_percent(old_price: f64, new_price: f64) -> f64 {
    if old_price == 0.0 {
        return 0.0;
    }
    ((new_price - old_price) / old_price) * 100.0
}

/// Расчёт прибыли в процентах
pub fn calculate_profit_percentage(initial: u64, final_amount: u64) -> f64 {
    if initial == 0 {
        return 0.0;
    }
    ((final_amount as f64 - initial as f64) / initial as f64) * 100.0
}

/// Проверка прибыльности с учётом минимального порога
pub fn is_profitable(
    initial_amount: u64,
    final_amount: u64,
    min_profit_lamports: u64,
) -> bool {
    final_amount >= initial_amount + min_profit_lamports
}

/// Расчёт общих комиссий транзакции
pub fn calculate_total_transaction_fees(
    base_fee: u64,
    priority_fee: u64,
    jito_tip: u64,
) -> u64 {
    base_fee + priority_fee + jito_tip
}

/// Расчёт точки безубыточности (breakeven) для арбитража
///
/// Возвращает минимальную финальную сумму для достижения нулевой прибыли
pub fn calculate_breakeven_amount(
    initial_amount: u64,
    transaction_fees: u64,
) -> u64 {
    initial_amount + transaction_fees
}

/// Расчёт эффективной APY для арбитража
///
/// # Параметры
/// - profit_lamports: Прибыль за одну сделку
/// - capital_lamports: Использованный капитал
/// - trades_per_day: Среднее количество сделок в день
pub fn calculate_effective_apy(
    profit_lamports: u64,
    capital_lamports: u64,
    trades_per_day: f64,
) -> f64 {
    if capital_lamports == 0 {
        return 0.0;
    }

    let daily_return = (profit_lamports as f64 / capital_lamports as f64) * trades_per_day;
    let annual_return = daily_return * 365.0;

    annual_return * 100.0 // В процентах
}

/// Расчёт impact на пул (price impact от свопа)
pub fn calculate_price_impact(
    reserve_in: u64,
    reserve_out: u64,
    amount_in: u64,
) -> f64 {
    let price_before = reserve_out as f64 / reserve_in as f64;
    let new_reserve_in = reserve_in + amount_in;
    let price_after = reserve_out as f64 / new_reserve_in as f64;

    ((price_after - price_before) / price_before).abs() * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amm_swap_output() {
        // Пул: 1000 USDC, 1000 SOL, комиссия 0.25%
        let output = calculate_amm_swap_output(
            1_000_000_000, // 1000 USDC
            1_000_000_000, // 1000 SOL
            100_000_000,   // 100 USDC in
            25,            // 0.25% fee
        ).unwrap();

        // Ожидаем ~90.7 SOL (с учётом комиссии и slippage)
        assert!(output > 90_000_000 && output < 100_000_000);
    }

    #[test]
    fn test_minimum_amount_out() {
        let expected = 1_000_000_000; // 1 SOL
        let min_out = calculate_minimum_amount_out(expected, 100); // 1% slippage

        assert_eq!(min_out, 990_000_000); // 0.99 SOL
    }

    #[test]
    fn test_price_change() {
        let change = calculate_price_change_percent(100.0, 110.0);
        assert!((change - 10.0).abs() < 0.01);

        let change_down = calculate_price_change_percent(100.0, 90.0);
        assert!((change_down + 10.0).abs() < 0.01);
    }

    #[test]
    fn test_profitability_check() {
        assert!(is_profitable(1_000_000, 1_100_000, 50_000)); // Прибыльно
        assert!(!is_profitable(1_000_000, 1_040_000, 50_000)); // Не прибыльно
    }

    #[test]
    fn test_apy_calculation() {
        // Прибыль 0.01 SOL с капитала 1 SOL, 10 сделок в день
        let apy = calculate_effective_apy(
            10_000_000,   // 0.01 SOL profit
            1_000_000_000, // 1 SOL capital
            10.0,         // 10 trades/day
        );

        // Ожидаем ~36.5% годовых
        assert!((apy - 36.5).abs() < 1.0);
    }

    #[test]
    fn test_price_impact() {
        let impact = calculate_price_impact(
            1_000_000_000, // 1000 tokens in reserve
            1_000_000_000, // 1000 tokens out reserve
            100_000_000,   // 100 tokens swap
        );

        // Impact должен быть ~9.09%
        assert!(impact > 9.0 && impact < 10.0);
    }
}