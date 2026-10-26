pub const FEE_DENOMINATOR_BPS: u64 = 10_000;
pub const DEPOSIT_FEE_BPS: u64 = 0; // 0% Deposit Fee (Immutable Zero-Friction Inflow)
pub const PRIVATE_SPEND_FEE_BPS: u64 = 45; // 0.45% (< 30 days)
pub const DEFAULT_EXECUTION_OVERHEAD_BPS: u64 = 1_500; // 15% execution gas volatility & DA buffer (DEC-054)

pub const THIRTY_DAYS_SECS: u64 = 30 * 24 * 60 * 60;
pub const SPEND_FEE_30DAY_BPS: u64 = 45; // DEC-028: Unified flat 45 bps (holding-time discount abolished)

/// Protocol spend fee in basis points.
/// In DEC-028, holding-time fee discounts are abolished to eliminate root age gaming and
/// maintain 100% predictable cash accounting ($10 - $3 = $7). Always returns 45 bps.
pub fn spend_fee_bps_for_holding(_holding_secs: u64) -> u64 {
    PRIVATE_SPEND_FEE_BPS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpendQuote {
    pub merchant_amount: u64,
    pub contract_amount: u64,
    pub protocol_fee: u64,
    pub gas_cost: u64,
    pub execution_overhead: u64, // 15% execution gas volatility & DA overhead buffer (DEC-054)
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

pub fn quote_execution_fee(gas_cost: u64, gas_overhead_bps: u64) -> Option<(u64, u64)> {
    let execution_overhead = fee_round_up(gas_cost, gas_overhead_bps)?;
    let execution_fee = gas_cost.checked_add(execution_overhead)?;
    Some((execution_overhead, execution_fee))
}

pub fn quote_execution_overhead(gas_cost: u64, gas_overhead_bps: u64) -> Option<(u64, u64)> {
    quote_execution_fee(gas_cost, gas_overhead_bps)
}

/// Quotes execution fee with a pass-through fee (such as Chainlink CCIP network fee).
/// Relayer gas overhead buffer is applied ONLY to the relayer gas cost, never to the pass-through fee.
pub fn quote_execution_fee_with_pass_through(
    relayer_gas: u64,
    pass_through: u64,
    gas_overhead_bps: u64,
) -> Option<(u64, u64)> {
    let execution_overhead = fee_round_up(relayer_gas, gas_overhead_bps)?;
    let execution_fee = relayer_gas
        .checked_add(execution_overhead)?
        .checked_add(pass_through)?;
    Some((execution_overhead, execution_fee))
}

pub fn quote_private_spend(
    merchant_amount: u64,
    gas_cost: u64,
    gas_overhead_bps: u64,
) -> Option<SpendQuote> {
    let contract_amount = merchant_amount;
    let protocol_fee = private_spend_fee(merchant_amount)?;
    let (execution_overhead, execution_fee) = quote_execution_fee(gas_cost, gas_overhead_bps)?;
    let user_total_debit = contract_amount
        .checked_add(protocol_fee)?
        .checked_add(execution_fee)?;

    Some(SpendQuote {
        merchant_amount,
        contract_amount,
        protocol_fee,
        gas_cost,
        execution_overhead,
        execution_fee,
        user_total_debit,
    })
}

/// Minimum note value eligible for standalone rollover (DEC-033: $2.00 USDC).
/// Prevents spam bot DoS that drains relayer mempool and CPU quota.
pub const MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC: u64 = 2_000_000; // 2.00 USDC

/// Number of Wei in 1 Gwei (10^9) for WETH 9-decimal normalization (DEC-034A §6).
pub const WEI_PER_GWEI: u128 = 1_000_000_000;

/// Converts Wei (18 decimals) to Gwei (9 decimals).
/// Returns None if there is a remainder modulo 10^9 (precision loss) or if value overflows u64.
pub fn wei_to_gwei(wei: u128) -> Option<u64> {
    if !wei.is_multiple_of(WEI_PER_GWEI) {
        return None;
    }
    let gwei = wei / WEI_PER_GWEI;
    u64::try_from(gwei).ok()
}

/// Converts Gwei (9 decimals) to Wei (18 decimals).
pub fn gwei_to_wei(gwei: u64) -> u128 {
    (gwei as u128) * WEI_PER_GWEI
}

/// Quotes a standalone rollover / refresh transaction (DEC-033).
/// In standalone rollover, merchant_amount = 0 and protocol_fee = 0 (0 bps).
/// Relayer receives gas reimbursement + 15% execution overhead buffer directly from the note being rolled over.
pub fn quote_standalone_rollover(gas_cost: u64, gas_overhead_bps: u64) -> Option<SpendQuote> {
    let (execution_overhead, execution_fee) = quote_execution_fee(gas_cost, gas_overhead_bps)?;
    Some(SpendQuote {
        merchant_amount: 0,
        contract_amount: 0,
        protocol_fee: 0,
        gas_cost,
        execution_overhead,
        execution_fee,
        user_total_debit: execution_fee,
    })
}

pub fn quote_private_spend_with_fee(
    merchant_amount: u64,
    gas_cost: u64,
    gas_overhead_bps: u64,
    fee_bps: u64,
) -> Option<SpendQuote> {
    let contract_amount = merchant_amount;
    let protocol_fee = fee_round_up(merchant_amount, fee_bps)?;
    let (execution_overhead, execution_fee) = quote_execution_fee(gas_cost, gas_overhead_bps)?;
    let user_total_debit = contract_amount
        .checked_add(protocol_fee)?
        .checked_add(execution_fee)?;

    Some(SpendQuote {
        merchant_amount,
        contract_amount,
        protocol_fee,
        gas_cost,
        execution_overhead,
        execution_fee,
        user_total_debit,
    })
}

/// Quotes cross-chain private spend where CCIP network fee is a pass-through fee.
/// Relayer gas overhead buffer is applied ONLY to `relayer_gas_cost`, NOT to `ccip_network_fee`.
pub fn quote_cross_chain_private_spend_with_fee(
    merchant_amount: u64,
    relayer_gas_cost: u64,
    ccip_network_fee: u64,
    gas_overhead_bps: u64,
    fee_bps: u64,
) -> Option<SpendQuote> {
    let contract_amount = merchant_amount;
    let protocol_fee = fee_round_up(merchant_amount, fee_bps)?;
    let (execution_overhead, execution_fee) = quote_execution_fee_with_pass_through(
        relayer_gas_cost,
        ccip_network_fee,
        gas_overhead_bps,
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
        execution_overhead,
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
        assert_eq!(quote.execution_overhead, 3_000);
        assert_eq!(quote.execution_fee, 23_000);
        assert_eq!(quote.user_total_debit, 100_473_000);
    }

    #[test]
    fn execution_fee_is_gas_plus_execution_overhead() {
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
    fn holding_time_fee_tiers_unified_flat() {
        // DEC-028: Holding-time discount is abolished; spend fee is flat 45 bps across all durations
        assert_eq!(spend_fee_bps_for_holding(0), 45);
        assert_eq!(spend_fee_bps_for_holding(59), 45);
        assert_eq!(spend_fee_bps_for_holding(THIRTY_DAYS_SECS - 1), 45);
        assert_eq!(spend_fee_bps_for_holding(THIRTY_DAYS_SECS), 45);
        assert_eq!(spend_fee_bps_for_holding(THIRTY_DAYS_SECS + 1), 45);
    }

    #[test]
    fn quote_with_custom_fee_bps() {
        let q45 = quote_private_spend_with_fee(100_000_000, 0, 0, 45).unwrap();
        assert_eq!(q45.protocol_fee, 450_000);

        let q40 = quote_private_spend_with_fee(100_000_000, 0, 0, 40).unwrap();
        assert_eq!(q40.protocol_fee, 400_000);
    }

    #[test]
    fn cross_chain_quote_applies_overhead_only_to_relayer_gas() {
        let relayer_gas = 20_000;
        let ccip_fee = 800_000;
        let overhead_bps = 1_500; // 15%
        let fee_bps = 45; // 0.45%
        let quote = quote_cross_chain_private_spend_with_fee(
            100_000_000,
            relayer_gas,
            ccip_fee,
            overhead_bps,
            fee_bps,
        )
        .unwrap();

        // 15% of 20_000 = 3_000. CCIP fee does NOT have execution overhead applied!
        assert_eq!(quote.execution_overhead, 3_000);
        assert_eq!(quote.gas_cost, 820_000);
        assert_eq!(quote.execution_fee, 20_000 + 3_000 + 800_000);
        assert_eq!(
            quote.user_total_debit,
            100_000_000 + 450_000 + 20_000 + 3_000 + 800_000
        );
    }

    #[test]
    fn standalone_rollover_quote_zero_protocol_fee_and_exact_relayer_reimbursement() {
        let gas_cost = 18_500; // ~$0.0185
        let overhead_bps = DEFAULT_EXECUTION_OVERHEAD_BPS; // 15% (1500 bps)
        let quote = quote_standalone_rollover(gas_cost, overhead_bps).unwrap();

        assert_eq!(quote.merchant_amount, 0);
        assert_eq!(quote.contract_amount, 0);
        assert_eq!(quote.protocol_fee, 0);
        // 15% of 18_500 = ceil(18500 * 1500 / 10000) = 2775
        assert_eq!(quote.execution_overhead, 2_775);
        assert_eq!(quote.execution_fee, 18_500 + 2_775);
        assert_eq!(quote.user_total_debit, quote.execution_fee);
    }

    #[test]
    fn dust_rollover_threshold_matches_dec033() {
        assert_eq!(MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC, 2_000_000); // Exactly $2.00 USDC
    }

    #[test]
    fn test_gwei_normalization_precision_bounds() {
        // Zero bound
        assert_eq!(wei_to_gwei(0), Some(0));
        assert_eq!(gwei_to_wei(0), 0);

        // 1 Gwei = 10^9 Wei
        assert_eq!(wei_to_gwei(1_000_000_000), Some(1));
        assert_eq!(gwei_to_wei(1), 1_000_000_000);

        // Clean values with no modulo remainder
        let five_weth_wei: u128 = 5 * 10u128.pow(18);
        let five_weth_gwei = 5_000_000_000u64;
        assert_eq!(wei_to_gwei(five_weth_wei), Some(five_weth_gwei));
        assert_eq!(gwei_to_wei(five_weth_gwei), five_weth_wei);

        // Fractional remainder rejected: 10^9 + 1 wei has remainder 1
        assert_eq!(wei_to_gwei(1_000_000_001), None);
        assert_eq!(wei_to_gwei(999_999_999), None);

        // Upper bound: u64::MAX Gwei in Wei
        let max_gwei = u64::MAX;
        let max_wei = gwei_to_wei(max_gwei);
        assert_eq!(wei_to_gwei(max_wei), Some(max_gwei));

        // Overflow of u64 capacity
        let overflow_wei = (u64::MAX as u128 + 1) * WEI_PER_GWEI;
        assert_eq!(wei_to_gwei(overflow_wei), None);
    }
}
