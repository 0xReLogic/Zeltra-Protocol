//! Liquid Vault Model for Treasury Management
//!
//! Implements simple full-reserve (100% liquid) treasury management.

use alloc::vec::Vec;
use alloy_primitives::{Address, U256};
use stylus_sdk::prelude::Call;

use crate::interfaces::IErc20;
use crate::storage::Zeltra;

impl Zeltra {
    /// Ensures that the contract has at least `required_amount` of stablecoin (USDC).
    pub(crate) fn ensure_liquidity(&mut self, required_amount: U256) -> Result<(), Vec<u8>> {
        self.ensure_asset_liquidity(1, required_amount)
    }

    /// Ensures that the contract has at least `required_raw_amount` of the given asset.
    pub(crate) fn ensure_asset_liquidity(
        &mut self,
        asset_id: u8,
        required_raw_amount: U256,
    ) -> Result<(), Vec<u8>> {
        #[cfg(not(test))]
        {
            let token_addr = self.asset_tokens.get(U256::from(asset_id));
            if token_addr == Address::ZERO {
                return Err(b"ASSET_NOT_CONFIGURED".to_vec());
            }
            let erc20 = IErc20::new(token_addr);
            let host = Self::runtime_host();
            let this_address = self.env_contract_address();

            let cash_balance = erc20
                .balance_of(&host, Call::new(), this_address)
                .map_err(|_| b"INSUFFICIENT_LIQUIDITY".to_vec())?;

            if cash_balance < required_raw_amount {
                return Err(b"INSUFFICIENT_LIQUIDITY".to_vec());
            }
        }
        Ok(())
    }

    /// Returns the total assets held by the contract for USDC.
    pub fn _total_assets(&mut self) -> Result<U256, Vec<u8>> {
        self._total_assets_of(1)
    }

    /// Returns the total assets held by the contract for a given asset (in circuit units).
    pub fn _total_assets_of(&mut self, asset_id: u8) -> Result<U256, Vec<u8>> {
        let asset_key = U256::from(asset_id);
        #[cfg(test)]
        {
            if let Some(balance) =
                crate::tests::MOCK_ASSET_BALANCES.with(|m| m.borrow().get(&asset_id).copied())
            {
                Ok(balance)
            } else {
                Ok(self.total_deposited_principals.get(asset_key))
            }
        }
        #[cfg(not(test))]
        {
            let token_addr = self.asset_tokens.get(asset_key);
            if token_addr == Address::ZERO {
                return Err(b"ASSET_NOT_CONFIGURED".to_vec());
            }
            let erc20 = IErc20::new(token_addr);
            let this_address = self.env_contract_address();
            let host = Self::runtime_host();
            let raw_cash = erc20
                .balance_of(&host, Call::new(), this_address)
                .unwrap_or(U256::ZERO);
            let cash = if asset_id == 0 {
                raw_cash / U256::from(1_000_000_000u64)
            } else {
                raw_cash
            };
            Ok(cash)
        }
    }

    /// Checks the solvency invariant for a specific asset:
    /// Balance(a) >= UserNoteLiabilities(a) + AccruedExecutionFees(a) + RealizedProtocolFees(a)
    pub(crate) fn _check_asset_solvency_invariant(&self, asset_id: u8) -> Result<(), Vec<u8>> {
        let asset_key = U256::from(asset_id);
        let total_liabilities = self
            .user_note_liabilities
            .get(asset_key)
            .checked_add(self.accrued_execution_fee_liabilities.get(asset_key))
            .ok_or_else(|| b"OVERFLOW".to_vec())?
            .checked_add(self.realized_protocol_fees.get(asset_key))
            .ok_or_else(|| b"OVERFLOW".to_vec())?;

        #[cfg(test)]
        {
            if let Some(balance) =
                crate::tests::MOCK_ASSET_BALANCES.with(|m| m.borrow().get(&asset_id).copied())
            {
                if balance < total_liabilities {
                    return Err(b"INSOLVENT".to_vec());
                }
            }
        }

        #[cfg(not(test))]
        {
            use stylus_sdk::prelude::Call;

            let token_addr = self.asset_tokens.get(asset_key);
            if token_addr == Address::ZERO {
                return Err(b"ASSET_NOT_CONFIGURED".to_vec());
            }

            let contract_address = self.env_contract_address();
            let erc20 = IErc20::new(token_addr);
            let host = Self::runtime_host();
            let raw_balance = erc20
                .balance_of(&host, Call::new(), contract_address)
                .map_err(|_| b"INVARIANT_BALANCE_CHECK_FAILED".to_vec())?;

            // Saldo WETH diukur dari IWeth9(weth9_address).balanceOf(this) / 10^9 (abaikan balance native ETH mentah)
            let balance = if asset_id == 0 {
                raw_balance / U256::from(1_000_000_000u64)
            } else {
                raw_balance
            };

            if balance < total_liabilities {
                return Err(b"INSOLVENT".to_vec());
            }
        }

        Ok(())
    }

    /// Claims any yield accumulated in the vault for USDC (total assets - total principal).
    pub fn _claim_accumulated_yield(&mut self) -> Result<U256, Vec<u8>> {
        self.check_owner()?;
        let total = self._total_assets_of(1)?;
        let principal = self.total_deposited_principals.get(U256::from(1));
        if total <= principal {
            return Ok(U256::ZERO);
        }
        let yield_amount = total - principal;

        self.ensure_asset_liquidity(1, yield_amount)?;

        let usdc_addr = self.asset_tokens.get(U256::from(1));
        if usdc_addr != Address::ZERO {
            let recipient = self.fee_recipient.get();
            self.asset_safe_transfer(usdc_addr, recipient, yield_amount)?;
        }

        // Emit FeeClaim event
        crate::events::emit_event(crate::events::FeeClaim {
            recipient: self.fee_recipient.get(),
            amount: yield_amount,
            fee_type: alloy_primitives::keccak256(b"YIELD"),
        });

        Ok(yield_amount)
    }
}
