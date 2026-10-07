pub const FEE_DENOMINATOR_BPS: u64 = 10_000;
pub const DEPOSIT_FEE_BPS: u64 = 0; // 0% Deposit Fee (Immutable Zero-Friction Inflow)
pub const PRIVATE_SPEND_FEE_BPS: u64 = 45; // 0.45% (< 30 days)
pub const DEFAULT_RELAYER_MARKUP_BPS: u64 = 1_500; // 15%

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
}
