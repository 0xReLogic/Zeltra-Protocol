use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct PrivateSpendQuote {
    merchant_amount: u64,
    contract_amount: u64,
    protocol_fee: u64,
    gas_cost: u64,
    relayer_markup: u64,
    execution_fee: u64,
    user_total_debit: u64,
}

#[wasm_bindgen]
impl PrivateSpendQuote {
    #[wasm_bindgen(getter)]
    pub fn merchant_amount(&self) -> u64 {
        self.merchant_amount
    }

    #[wasm_bindgen(getter)]
    pub fn contract_amount(&self) -> u64 {
        self.contract_amount
    }

    #[wasm_bindgen(getter)]
    pub fn protocol_fee(&self) -> u64 {
        self.protocol_fee
    }

    #[wasm_bindgen(getter)]
    pub fn execution_fee(&self) -> u64 {
        self.execution_fee
    }

    #[wasm_bindgen(getter)]
    pub fn gas_cost(&self) -> u64 {
        self.gas_cost
    }

    #[wasm_bindgen(getter)]
    pub fn relayer_markup(&self) -> u64 {
        self.relayer_markup
    }

    #[wasm_bindgen(getter)]
    pub fn user_total_debit(&self) -> u64 {
        self.user_total_debit
    }
}

#[wasm_bindgen]
pub fn quote_private_spend_for_merchant_amount(
    merchant_amount: u64,
    gas_cost: u64,
    relayer_markup_bps: u64,
) -> Result<PrivateSpendQuote, JsValue> {
    let quote =
        nimbus_core::quote_private_spend(merchant_amount, gas_cost, relayer_markup_bps)
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;

    Ok(PrivateSpendQuote {
        merchant_amount: quote.merchant_amount,
        contract_amount: quote.contract_amount,
        protocol_fee: quote.protocol_fee,
        gas_cost: quote.gas_cost,
        relayer_markup: quote.relayer_markup,
        execution_fee: quote.execution_fee,
        user_total_debit: quote.user_total_debit,
    })
}

#[wasm_bindgen]
pub fn gross_up_private_spend_amount(merchant_amount: u64) -> Result<u64, JsValue> {
    nimbus_core::gross_up_private_spend_amount(merchant_amount)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))
}

#[wasm_bindgen]
pub fn private_spend_protocol_fee(contract_amount: u64) -> Result<u64, JsValue> {
    nimbus_core::private_spend_fee(contract_amount)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))
}
