//! Liquid Vault Model for Treasury Management
//!
//! Implements simple full-reserve (100% liquid) treasury management.

use alloc::vec::Vec;
use alloy_primitives::{Address, U256};
use stylus_sdk::prelude::Call;

use crate::interfaces::IErc20;
use crate::storage::Nimbus;

impl Nimbus {
    /// Ensures that the contract has at least `required_amount` of stablecoin.
    /// In a 100% liquid full-reserve model, we simply verify our direct cash balance.
    pub(crate) fn ensure_liquidity(&mut self, required_amount: U256) -> Result<(), Vec<u8>> {
        #[cfg(not(test))]
        {
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let host = Self::runtime_host();
            let this_address = self.env_contract_address();

            let cash_balance = erc20.balance_of(&host, Call::new(), this_address)?;

            if cash_balance < required_amount {
                return Err(b"INSUFFICIENT_LIQUIDITY".to_vec());
            }
        }
        Ok(())
    }

    /// Returns the total assets held by the contract.
    /// In a 100% liquid full-reserve model, this is simply the stablecoin balance.
    pub fn _total_assets(&mut self) -> Result<U256, Vec<u8>> {
        #[cfg(test)]
        {
            Ok(self.total_deposited_principal.get())
        }
        #[cfg(not(test))]
        {
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let this_address = self.env_contract_address();
            let host = Self::runtime_host();
            let cash = erc20
                .balance_of(&host, Call::new(), this_address)
                .unwrap_or(U256::ZERO);
            Ok(cash)
        }
    }

    /// Claims any yield accumulated in the vault (total assets - total principal).
    pub fn _claim_accumulated_yield(&mut self) -> Result<U256, Vec<u8>> {
        self.check_owner()?;
        let total = self._total_assets()?;
        let principal = self.total_deposited_principal.get();
        if total <= principal {
            return Ok(U256::ZERO);
        }
        let yield_amount = total - principal;

        self.ensure_liquidity(yield_amount)?;

        #[cfg(not(test))]
        {
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let recipient = self.fee_recipient.get();
            let host = Self::runtime_host();
            let success =
                erc20.transfer(&host, Call::new_mutating(self), recipient, yield_amount)?;
            if !success {
                return Err(b"YIELD_TRANSFER_FAILED".to_vec());
            }
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
