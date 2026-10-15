//! Private helper methods for Zeltra contract

use crate::storage::Zeltra;
use alloc::vec::Vec;
use alloy_primitives::Address;

impl Zeltra {
    #[inline(always)]
    pub(crate) fn runtime_host() -> stylus_sdk::host::WasmVM {
        stylus_sdk::host::WasmVM {}
    }

    #[inline(always)]
    pub(crate) fn msg_sender(&self) -> Address {
        #[cfg(test)]
        {
            crate::tests::MSG_SENDER.with(|s| *s.borrow())
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::MessageAccess;
            Self::runtime_host().msg_sender()
        }
    }

    #[inline(always)]
    pub(crate) fn block_timestamp(&self) -> u64 {
        #[cfg(test)]
        {
            crate::tests::BLOCK_TIMESTAMP.with(|t| *t.borrow())
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::BlockAccess;
            Self::runtime_host().block_timestamp()
        }
    }

    #[inline(always)]
    pub(crate) fn env_chain_id(&self) -> u64 {
        #[cfg(test)]
        {
            1337
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::ChainAccess;
            Self::runtime_host().chain_id()
        }
    }

    #[inline(always)]
    pub(crate) fn env_contract_address(&self) -> Address {
        #[cfg(test)]
        {
            Address::ZERO
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::AccountAccess;
            Self::runtime_host().contract_address()
        }
    }

    pub(crate) fn check_owner(&self) -> Result<(), Vec<u8>> {
        if self.owner.get() != self.msg_sender() {
            return Err(b"NOT_OWNER".to_vec());
        }
        Ok(())
    }

    pub(crate) fn check_not_paused(&self) -> Result<(), Vec<u8>> {
        if self.paused.get() {
            return Err(b"CONTRACT_PAUSED".to_vec());
        }
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn msg_value(&self) -> alloy_primitives::U256 {
        #[cfg(test)]
        {
            crate::tests::MSG_VALUE.with(|v| *v.borrow())
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::MessageAccess;
            Self::runtime_host().msg_value()
        }
    }

    #[inline(always)]
    pub(crate) fn erc20_transfer_from(
        &mut self,
        client: Address,
        this_address: Address,
        amount: alloy_primitives::U256,
    ) -> Result<bool, Vec<u8>> {
        #[cfg(test)]
        {
            let _ = (client, this_address, amount);
            crate::tests::ERC20_TRANSFER_FROM_RESULT.with(|r| r.borrow().clone())
        }
        #[cfg(not(test))]
        {
            use crate::interfaces::IErc20;
            use stylus_sdk::prelude::Call;

            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let host = Self::runtime_host();
            erc20
                .transfer_from(
                    &host,
                    Call::new_mutating(self),
                    client,
                    this_address,
                    amount,
                )
                .map_err(|_| b"TRANSFER_FROM_FAILED".to_vec())
        }
    }

    pub(crate) fn asset_safe_transfer_from(
        &mut self,
        token_addr: Address,
        client: Address,
        this_address: Address,
        raw_amount: alloy_primitives::U256,
    ) -> Result<(), Vec<u8>> {
        #[cfg(test)]
        {
            let _ = (token_addr, client, this_address, raw_amount);
            let ok = crate::tests::ERC20_TRANSFER_FROM_RESULT.with(|r| r.borrow().clone())?;
            if !ok {
                return Err(b"TRANSFER_FROM_FAILED".to_vec());
            }
            Ok(())
        }
        #[cfg(not(test))]
        {
            use crate::interfaces::IErc20;
            use stylus_sdk::prelude::Call;

            let erc20 = IErc20::new(token_addr);
            let host = Self::runtime_host();
            let success = erc20
                .transfer_from(
                    &host,
                    Call::new_mutating(self),
                    client,
                    this_address,
                    raw_amount,
                )
                .map_err(|_| b"TRANSFER_FROM_FAILED".to_vec())?;
            if !success {
                return Err(b"TRANSFER_FROM_FAILED".to_vec());
            }
            Ok(())
        }
    }

    pub(crate) fn asset_safe_transfer(
        &mut self,
        token_addr: Address,
        recipient: Address,
        raw_amount: alloy_primitives::U256,
    ) -> Result<(), Vec<u8>> {
        #[cfg(test)]
        {
            let _ = (token_addr, recipient, raw_amount);
            let res = crate::tests::ERC20_TRANSFER_RESULT.with(|r| r.borrow().clone())?;
            if !res {
                return Err(b"TRANSFER_FAILED".to_vec());
            }
            Ok(())
        }
        #[cfg(not(test))]
        {
            use crate::interfaces::IErc20;
            use stylus_sdk::prelude::Call;

            let erc20 = IErc20::new(token_addr);
            let host = Self::runtime_host();
            let success = erc20
                .transfer(&host, Call::new_mutating(self), recipient, raw_amount)
                .map_err(|_| b"TRANSFER_FAILED".to_vec())?;
            if !success {
                return Err(b"TRANSFER_FAILED".to_vec());
            }
            Ok(())
        }
    }

    pub(crate) fn weth9_deposit(
        &mut self,
        weth9_addr: Address,
        amount_wei: alloy_primitives::U256,
    ) -> Result<(), Vec<u8>> {
        #[cfg(test)]
        {
            crate::tests::WETH9_DEPOSIT_CALLS.with(|c| {
                c.borrow_mut().push((weth9_addr, amount_wei));
            });
            Ok(())
        }
        #[cfg(not(test))]
        {
            use crate::interfaces::IWeth9;
            use stylus_sdk::prelude::Call;

            let iweth = IWeth9::new(weth9_addr);
            let host = Self::runtime_host();
            iweth
                .deposit(&host, Call::new_mutating(self).value(amount_wei))
                .map_err(|_| b"WETH_WRAP_FAILED".to_vec())?;
            Ok(())
        }
    }
}
