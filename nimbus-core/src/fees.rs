pub const FEE_DENOMINATOR_BPS: u64 = 10_000;
pub const DEPOSIT_FEE_BPS: u64 = 0; // 0% Deposit Fee (Immutable Zero-Friction Inflow)
pub const PRIVATE_SPEND_FEE_BPS: u64 = 45; // 0.45% (< 30 days)
pub const DEFAULT_RELAYER_MARKUP_BPS: u64 = 1_500; // 15%

pub const THIRTY_DAYS_SECS: u64 = 30 * 24 * 60 * 60;
pub const SPEND_FEE_30DAY_BPS: u64 = 40; // 0.40% (>= 30 days / 1 month hold legacy default)

// DEC-027 Dynamic Liquidity-Outflow Economic Fee Model (Kinked Reserve-Velocity Curve)
pub const DYNAMIC_FEE_FLOOR_BPS: u64 = 35; // 0.35% (R_floor: thick TVL / low velocity)
pub const DYNAMIC_FEE_BASE_SLOPE_BPS: u64 = 10; // 0.10% (R_base: up to 45 bps at U_optimal)
pub const DYNAMIC_FEE_SURGE_SLOPE_BPS: u64 = 25; // 0.25% (R_surge: up to 70 bps max at U = 1.0)
pub const DYNAMIC_FEE_OPTIMAL_UTILIZATION_BPS: u64 = 2_000; // 20.00% daily volume / TVL
pub const DYNAMIC_FEE_CEILING_BPS: u64 = 70; // 0.70% absolute maximum surge fee
pub const HOLDING_DISCOUNT_30DAY_BPS: u64 = 5; // 5 bps discount for >= 30 days hold

pub fn spend_fee_bps_for_holding(holding_secs: u64) -> u64 {
    if holding_secs >= THIRTY_DAYS_SECS {
        SPEND_FEE_30DAY_BPS
    } else {
        PRIVATE_SPEND_FEE_BPS
    }
}

/// Calculates dynamic outflow spend fee rate in basis points (DEC-027 Kinked Reserve-Velocity Curve).
///
/// Mathematical Model:
/// - Let U = min(1.0, rolling_outflow_24h / pool_tvl)
/// - If U <= U_optimal (20%):
///   Fee = R_floor + ceil(U / U_optimal * R_base)  [35 bps -> 45 bps]
/// - If U > U_optimal (20% -> 100%):
///   Fee = R_floor + R_base + ceil((U - U_optimal) / (1 - U_optimal) * R_surge)  [45 bps -> 70 bps]
/// - If holding_secs >= 30 days, a 5 bps discount is applied (capped at floor - 5 = 30 bps min).
/// - If pool_tvl == 0, defaults fail-closed to maximum surge fee (70 bps, or 65 bps with discount).
pub fn calculate_dynamic_outflow_fee_bps(
    rolling_outflow_usdc: u64,
    pool_tvl_usdc: u64,
    holding_secs: u64,
) -> u64 {
    let unadjusted_fee = if pool_tvl_usdc == 0 {
        DYNAMIC_FEE_CEILING_BPS
    } else if rolling_outflow_usdc == 0 {
        DYNAMIC_FEE_FLOOR_BPS
    } else {
        // Calculate U in basis points [0..10_000] using u128 to prevent overflow
        let u_bps = (rolling_outflow_usdc as u128)
            .saturating_mul(FEE_DENOMINATOR_BPS as u128)
            .div_ceil(pool_tvl_usdc as u128);
        let u_bps = (u_bps.min(FEE_DENOMINATOR_BPS as u128)) as u64;

        if u_bps <= DYNAMIC_FEE_OPTIMAL_UTILIZATION_BPS {
            // First slope: R_floor + ceil(u_bps * R_base / U_optimal)
            let slope = ceil_div(
                u_bps.saturating_mul(DYNAMIC_FEE_BASE_SLOPE_BPS),
                DYNAMIC_FEE_OPTIMAL_UTILIZATION_BPS,
            )
            .unwrap_or(0);
            DYNAMIC_FEE_FLOOR_BPS.saturating_add(slope)
        } else {
            // Second slope: R_floor + R_base + ceil((u_bps - U_optimal) * R_surge / (10_000 - U_optimal))
            let excess_u = u_bps.saturating_sub(DYNAMIC_FEE_OPTIMAL_UTILIZATION_BPS);
            let excess_range =
                FEE_DENOMINATOR_BPS.saturating_sub(DYNAMIC_FEE_OPTIMAL_UTILIZATION_BPS);
            let surge_slope = ceil_div(
                excess_u.saturating_mul(DYNAMIC_FEE_SURGE_SLOPE_BPS),
                excess_range,
            )
            .unwrap_or(0);
            DYNAMIC_FEE_FLOOR_BPS
                .saturating_add(DYNAMIC_FEE_BASE_SLOPE_BPS)
                .saturating_add(surge_slope)
        }
    };

    let bounded_fee = unadjusted_fee.clamp(DYNAMIC_FEE_FLOOR_BPS, DYNAMIC_FEE_CEILING_BPS);

    if holding_secs >= THIRTY_DAYS_SECS {
        let min_discounted = DYNAMIC_FEE_FLOOR_BPS.saturating_sub(HOLDING_DISCOUNT_30DAY_BPS);
        bounded_fee
            .saturating_sub(HOLDING_DISCOUNT_30DAY_BPS)
            .max(min_discounted)
    } else {
        bounded_fee
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpendQuote {
    pub merchant_amount: u64,
    pub contract_amount: u64,
    pub protocol_fee: u64,
    pub gas_cost: u64,
    pub relayer_markup: u64,
    pub execution_fee: u64,
    pub user_total_debit: u64,
}

pub fn ceil_div(numerator: u64, denominator: u64) -> Option<u64> {
    if denominator == 0 {
        return None;
    }
    numerator
        .checked_add(denominator.checked_sub(1)?)
        .map(|value| value / denominator)
}

pub fn fee_round_up(amount: u64, fee_bps: u64) -> Option<u64> {
    let numerator = amount.checked_mul(fee_bps)?;
    ceil_div(numerator, FEE_DENOMINATOR_BPS)
}

pub fn net_after_fee(amount: u64, fee_bps: u64) -> Option<u64> {
    amount.checked_sub(fee_round_up(amount, fee_bps)?)
}

pub fn gross_up_for_exact_net(net_amount: u64, fee_bps: u64) -> Option<u64> {
    if fee_bps >= FEE_DENOMINATOR_BPS {
        return None;
    }
    let keep_bps = FEE_DENOMINATOR_BPS.checked_sub(fee_bps)?;
    let numerator = net_amount.checked_mul(FEE_DENOMINATOR_BPS)?;
    let mut gross = ceil_div(numerator, keep_bps)?;

    while net_after_fee(gross, fee_bps)? < net_amount {
        gross = gross.checked_add(1)?;
    }
    Some(gross)
}

pub fn deposit_fee(amount: u64) -> Option<u64> {
    fee_round_up(amount, DEPOSIT_FEE_BPS)
}

pub fn private_spend_fee(amount: u64) -> Option<u64> {
    fee_round_up(amount, PRIVATE_SPEND_FEE_BPS)
}

pub fn gross_up_private_spend_amount(merchant_amount: u64) -> Option<u64> {
    gross_up_for_exact_net(merchant_amount, PRIVATE_SPEND_FEE_BPS)
}

pub fn quote_execution_fee(gas_cost: u64, relayer_markup_bps: u64) -> Option<(u64, u64)> {
    let relayer_markup = fee_round_up(gas_cost, relayer_markup_bps)?;
    let execution_fee = gas_cost.checked_add(relayer_markup)?;
    Some((relayer_markup, execution_fee))
}

/// Quotes execution fee with a pass-through fee (such as Chainlink CCIP network fee).
/// Relayer markup is applied ONLY to the relayer gas cost, never to the pass-through fee.
pub fn quote_execution_fee_with_pass_through(
    relayer_gas: u64,
    pass_through: u64,
    relayer_markup_bps: u64,
) -> Option<(u64, u64)> {
    let relayer_markup = fee_round_up(relayer_gas, relayer_markup_bps)?;
    let execution_fee = relayer_gas
        .checked_add(relayer_markup)?
        .checked_add(pass_through)?;
    Some((relayer_markup, execution_fee))
}

pub fn quote_private_spend(
    merchant_amount: u64,
    gas_cost: u64,
    relayer_markup_bps: u64,
) -> Option<SpendQuote> {
    let contract_amount = merchant_amount;
    let protocol_fee = private_spend_fee(merchant_amount)?;
    let (relayer_markup, execution_fee) = quote_execution_fee(gas_cost, relayer_markup_bps)?;
    let user_total_debit = contract_amount
        .checked_add(protocol_fee)?
        .checked_add(execution_fee)?;

    Some(SpendQuote {
        merchant_amount,
        contract_amount,
        protocol_fee,
        gas_cost,
        relayer_markup,
        execution_fee,
        user_total_debit,
    })
}

pub fn quote_private_spend_with_fee(
    merchant_amount: u64,
    gas_cost: u64,
    relayer_markup_bps: u64,
    fee_bps: u64,
) -> Option<SpendQuote> {
    let contract_amount = merchant_amount;
    let protocol_fee = fee_round_up(merchant_amount, fee_bps)?;
    let (relayer_markup, execution_fee) = quote_execution_fee(gas_cost, relayer_markup_bps)?;
    let user_total_debit = contract_amount
        .checked_add(protocol_fee)?
        .checked_add(execution_fee)?;

    Some(SpendQuote {
        merchant_amount,
        contract_amount,
        protocol_fee,
        gas_cost,
        relayer_markup,
        execution_fee,
        user_total_debit,
    })
}

/// Quotes cross-chain private spend where CCIP network fee is a pass-through fee.
/// Relayer markup is applied ONLY to `relayer_gas_cost`, NOT to `ccip_network_fee`.
pub fn quote_cross_chain_private_spend_with_fee(
    merchant_amount: u64,
    relayer_gas_cost: u64,
    ccip_network_fee: u64,
    relayer_markup_bps: u64,
    fee_bps: u64,
) -> Option<SpendQuote> {
    let contract_amount = merchant_amount;
    let protocol_fee = fee_round_up(merchant_amount, fee_bps)?;
    let (relayer_markup, execution_fee) = quote_execution_fee_with_pass_through(
        relayer_gas_cost,
        ccip_network_fee,
        relayer_markup_bps,
    )?;
    let gas_cost = relayer_gas_cost.checked_add(ccip_network_fee)?;
    let user_total_debit = contract_amount
        .checked_add(protocol_fee)?
        .checked_add(execution_fee)?;

    Some(SpendQuote {
        merchant_amount,
        contract_amount,
        protocol_fee,
        gas_cost,
        relayer_markup,
        execution_fee,
        user_total_debit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_fee_matches_contract_rounding() {
        assert_eq!(deposit_fee(10_000_000), Some(0));
        assert_eq!(deposit_fee(20_000_000), Some(0));
        assert_eq!(deposit_fee(1), Some(0));
        assert_eq!(deposit_fee(0), Some(0));
    }

    #[test]
    fn private_spend_fee_matches_contract_rounding() {
        assert_eq!(private_spend_fee(10_000_000), Some(45_000));
        assert_eq!(private_spend_fee(1), Some(1));
        assert_eq!(private_spend_fee(0), Some(0));
    }

    #[test]
    fn quote_preserves_exact_merchant_payout() {
        let quote = quote_private_spend(100_000_000, 20_000, 1_500).unwrap();

        assert_eq!(quote.merchant_amount, 100_000_000);
        assert_eq!(quote.contract_amount, 100_000_000);
        assert_eq!(quote.protocol_fee, 450_000);
        assert_eq!(quote.gas_cost, 20_000);
        assert_eq!(quote.relayer_markup, 3_000);
        assert_eq!(quote.execution_fee, 23_000);
        assert_eq!(quote.user_total_debit, 100_473_000);
    }

    #[test]
    fn execution_fee_is_gas_plus_relayer_markup() {
        assert_eq!(quote_execution_fee(20_000, 1_500), Some((3_000, 23_000)));
        assert_eq!(quote_execution_fee(1, 1_500), Some((1, 2)));
        assert_eq!(quote_execution_fee(0, 1_500), Some((0, 0)));
    }

    #[test]
    fn gross_up_handles_rounding_boundaries() {
        for merchant_amount in [1, 2, 999, 1_000, 9_985_000, 10_000_000] {
            let gross = gross_up_private_spend_amount(merchant_amount).unwrap();
            assert!(net_after_fee(gross, PRIVATE_SPEND_FEE_BPS).unwrap() >= merchant_amount);
            if gross > 0 {
                assert!(net_after_fee(gross - 1, PRIVATE_SPEND_FEE_BPS).unwrap() < merchant_amount);
            }
        }
    }

    #[test]
    fn holding_time_fee_tiers() {
        assert_eq!(spend_fee_bps_for_holding(0), 45);
        assert_eq!(spend_fee_bps_for_holding(59), 45);
        assert_eq!(spend_fee_bps_for_holding(THIRTY_DAYS_SECS - 1), 45);
        assert_eq!(spend_fee_bps_for_holding(THIRTY_DAYS_SECS), 40);
        assert_eq!(spend_fee_bps_for_holding(THIRTY_DAYS_SECS + 1), 40);
    }

    #[test]
    fn quote_with_custom_fee_bps() {
        let q45 = quote_private_spend_with_fee(100_000_000, 0, 0, 45).unwrap();
        assert_eq!(q45.protocol_fee, 450_000);

        let q40 = quote_private_spend_with_fee(100_000_000, 0, 0, 40).unwrap();
        assert_eq!(q40.protocol_fee, 400_000);
    }

    #[test]
    fn cross_chain_quote_applies_markup_only_to_relayer_gas() {
        let relayer_gas = 20_000;
        let ccip_fee = 800_000;
        let markup_bps = 1_500; // 15%
        let fee_bps = 45; // 0.45%
        let quote = quote_cross_chain_private_spend_with_fee(
            100_000_000,
            relayer_gas,
            ccip_fee,
            markup_bps,
            fee_bps,
        )
        .unwrap();

        // 15% of 20_000 = 3_000. CCIP fee is NOT marked up!
        assert_eq!(quote.relayer_markup, 3_000);
        assert_eq!(quote.gas_cost, 820_000);
        assert_eq!(quote.execution_fee, 20_000 + 3_000 + 800_000);
        assert_eq!(
            quote.user_total_debit,
            100_000_000 + 450_000 + 20_000 + 3_000 + 800_000
        );
    }

    #[test]
    fn test_dynamic_fee_zero_tvl_fallback() {
        // Zero TVL should fail-closed to maximum ceiling (70 bps)
        assert_eq!(calculate_dynamic_outflow_fee_bps(1_000, 0, 0), 70);
        // With holding discount (>= 30 days): 70 - 5 = 65 bps
        assert_eq!(
            calculate_dynamic_outflow_fee_bps(1_000, 0, THIRTY_DAYS_SECS),
            65
        );
    }

    #[test]
    fn test_dynamic_fee_zero_outflow_floor() {
        // Zero outflow with healthy TVL (100k USDC) yields floor fee (35 bps)
        let tvl = 100_000_000_000;
        assert_eq!(calculate_dynamic_outflow_fee_bps(0, tvl, 0), 35);
        // With holding discount: 35 - 5 = 30 bps
        assert_eq!(
            calculate_dynamic_outflow_fee_bps(0, tvl, THIRTY_DAYS_SECS),
            30
        );
    }

    #[test]
    fn test_dynamic_fee_optimal_utilization_curve() {
        let tvl = 100_000_000_000; // 100,000 USDC

        // 10% daily utilization (halfway to U_optimal 20%):
        // 35 + ceil(1000 * 10 / 2000) = 35 + 5 = 40 bps
        let outflow_10pct = 10_000_000_000;
        assert_eq!(calculate_dynamic_outflow_fee_bps(outflow_10pct, tvl, 0), 40);

        // 20% daily utilization (exactly U_optimal 20%):
        // 35 + ceil(2000 * 10 / 2000) = 35 + 10 = 45 bps (matches legacy default!)
        let outflow_20pct = 20_000_000_000;
        assert_eq!(calculate_dynamic_outflow_fee_bps(outflow_20pct, tvl, 0), 45);
        // With holding discount: 45 - 5 = 40 bps (matches legacy 30-day default!)
        assert_eq!(
            calculate_dynamic_outflow_fee_bps(outflow_20pct, tvl, THIRTY_DAYS_SECS),
            40
        );
    }

    #[test]
    fn test_dynamic_fee_surge_pricing_above_optimal() {
        let tvl = 100_000_000_000; // 100,000 USDC

        // 60% daily utilization (U_bps = 6000, excess = 4000 out of 8000 range):
        // 35 + 10 + ceil(4000 * 25 / 8000) = 45 + 13 = 58 bps
        let outflow_60pct = 60_000_000_000;
        assert_eq!(calculate_dynamic_outflow_fee_bps(outflow_60pct, tvl, 0), 58);

        // 100% daily utilization (extreme bank-run stress):
        // 45 + ceil(8000 * 25 / 8000) = 45 + 25 = 70 bps (capped at ceiling)
        let outflow_100pct = 100_000_000_000;
        assert_eq!(
            calculate_dynamic_outflow_fee_bps(outflow_100pct, tvl, 0),
            70
        );

        // Even if outflow exceeds TVL, capped at 70 bps ceiling
        let outflow_200pct = 200_000_000_000;
        assert_eq!(
            calculate_dynamic_outflow_fee_bps(outflow_200pct, tvl, 0),
            70
        );
    }

    #[test]
    fn test_dynamic_fee_overflow_protection_large_values() {
        // Very large TVL ($100B USDC in micro-units = 10^17)
        let huge_tvl = 100_000_000_000_000_000;
        let huge_outflow = 20_000_000_000_000_000; // 20%
        assert_eq!(
            calculate_dynamic_outflow_fee_bps(huge_outflow, huge_tvl, 0),
            45
        );
    }
}
