// bot/src/arbitrage/pool_math.rs

use anyhow::Result;

/// Расчет выхода для пула CPMM (Constant Product Market Maker)
pub fn calculate_cpmm_output(
    reserve_in: u64,
    reserve_out: u64,
    amount_in: u64,
    fee_bps: u16,
) -> Result<u64> {
    if reserve_in == 0 || reserve_out == 0 {
        anyhow::bail!("Нулевые резервы в CPMM пуле");
    }

    let fee_multiplier = 1.0 - (fee_bps as f64 / 10000.0);
    let amount_in_with_fee = (amount_in as f64) * fee_multiplier;

    let numerator = (reserve_out as f64) * amount_in_with_fee;
    let denominator = (reserve_in as f64) + amount_in_with_fee;

    let amount_out = numerator / denominator;
    Ok(amount_out as u64)
}

/// Расчет выхода для пула CLMM (Concentrated Liquidity)
pub fn calculate_clmm_output(
    liquidity: u128,
    sqrt_price_current: u128,
    sqrt_price_next: u128,
    amount_in: u64,
    fee_bps: u16,
) -> Result<u64> {
    if liquidity == 0 {
        anyhow::bail!("Нулевая ликвидность в CLMM пуле");
    }

    let fee_multiplier = 1.0 - (fee_bps as f64 / 10000.0);
    let _amount_in_with_fee = (amount_in as f64) * fee_multiplier;

    let l_f64 = liquidity as f64;
    let sqrt_p_current = sqrt_price_current as f64;
    let sqrt_p_next = sqrt_price_next as f64;

    let delta_y = l_f64 * (sqrt_p_next - sqrt_p_current) / (sqrt_p_current * sqrt_p_next);
    let amount_out = delta_y * fee_multiplier;

    Ok(amount_out as u64)
}

/// Расчет выхода для пула DLMM (Dynamic Liquidity Market Maker)
pub fn calculate_dlmm_output(
    bin_liquidity: u64,
    bin_price: f64,
    composition: f64,
    amount_in: u64,
    base_fee_bps: u16,
    variable_fee_bps: u16,
) -> Result<u64> {
    if bin_liquidity == 0 {
        anyhow::bail!("Нулевая ликвидность в DLMM бине");
    }

    let total_fee_bps = base_fee_bps + variable_fee_bps;
    let fee_multiplier = 1.0 - (total_fee_bps as f64 / 10000.0);

    let l_f64 = bin_liquidity as f64;
    let reserve_y = composition * l_f64;
    let reserve_x = l_f64 / (bin_price * (1.0 - composition));

    let amount_in_with_fee = (amount_in as f64) * fee_multiplier;
    let numerator = reserve_y * amount_in_with_fee;
    let denominator = reserve_x + amount_in_with_fee;

    let amount_out = numerator / denominator;
    Ok(amount_out as u64)
}

/// Расчет минимального выхода с учетом slippage
pub fn calculate_minimum_amount_out(
    expected_amount: u64,
    slippage_bps: u16,
) -> u64 {
    let slippage_multiplier = 1.0 - (slippage_bps as f64 / 10000.0);
    ((expected_amount as f64) * slippage_multiplier) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpmm_calculation() {
        let output = calculate_cpmm_output(
            1_000_000_000,
            1_000_000_000,
            100_000_000,
            25,
        ).unwrap();

        assert!(output > 90_000_000 && output < 100_000_000);
    }
}