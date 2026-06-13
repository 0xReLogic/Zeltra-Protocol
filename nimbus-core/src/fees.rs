pub const FEE_DENOMINATOR_BPS: u64 = 10_000;
pub const DEPOSIT_FEE_BPS: u64 = 20;
pub const PRIVATE_SPEND_FEE_BPS: u64 = 25;
pub const DEFAULT_RELAYER_MARKUP_BPS: u64 = 1_500;

pub const SEVEN_DAYS_SECS: u64 = 7 * 24 * 60 * 60;
pub const THIRTY_DAYS_SECS: u64 = 30 * 24 * 60 * 60;
pub const SPEND_FEE_7DAY_BPS: u64 = 20;
pub const SPEND_FEE_30DAY_BPS: u64 = 10;

pub fn spend_fee_bps_for_holding(holding_secs: u64) -> u64 {
    if holding_secs >= THIRTY_DAYS_SECS {
        SPEND_FEE_30DAY_BPS
    } else if holding_secs >= SEVEN_DAYS_SECS {
        SPEND_FEE_7DAY_BPS
    } else {
        PRIVATE_SPEND_FEE_BPS
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_fee_matches_contract_rounding() {
        assert_eq!(deposit_fee(10_000_000), Some(20_000));
        assert_eq!(deposit_fee(20_000_000), Some(40_000));
        assert_eq!(deposit_fee(1), Some(1));
        assert_eq!(deposit_fee(0), Some(0));
    }

    #[test]
    fn private_spend_fee_matches_contract_rounding() {
        assert_eq!(private_spend_fee(10_000_000), Some(25_000));
        assert_eq!(private_spend_fee(1), Some(1));
        assert_eq!(private_spend_fee(0), Some(0));
    }

    #[test]
    fn quote_preserves_exact_merchant_payout() {
        let quote = quote_private_spend(100_000_000, 20_000, 1_500).unwrap();

        assert_eq!(quote.merchant_amount, 100_000_000);
        assert_eq!(quote.contract_amount, 100_000_000);
        assert_eq!(quote.protocol_fee, 250_000);
        assert_eq!(quote.gas_cost, 20_000);
        assert_eq!(quote.relayer_markup, 3_000);
        assert_eq!(quote.execution_fee, 23_000);
        assert_eq!(quote.user_total_debit, 100_273_000);
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
        assert_eq!(spend_fee_bps_for_holding(0), 25);
        assert_eq!(spend_fee_bps_for_holding(59), 25);
        assert_eq!(spend_fee_bps_for_holding(SEVEN_DAYS_SECS), 20);
        assert_eq!(spend_fee_bps_for_holding(SEVEN_DAYS_SECS + 1), 20);
        assert_eq!(spend_fee_bps_for_holding(THIRTY_DAYS_SECS), 10);
        assert_eq!(spend_fee_bps_for_holding(THIRTY_DAYS_SECS + 1), 10);
    }

    #[test]
    fn quote_with_custom_fee_bps() {
        let q25 = quote_private_spend_with_fee(100_000_000, 0, 0, 25).unwrap();
        assert_eq!(q25.protocol_fee, 250_000);

        let q20 = quote_private_spend_with_fee(100_000_000, 0, 0, 20).unwrap();
        assert_eq!(q20.protocol_fee, 200_000);

        let q10 = quote_private_spend_with_fee(100_000_000, 0, 0, 10).unwrap();
        assert_eq!(q10.protocol_fee, 100_000);
    }
}
