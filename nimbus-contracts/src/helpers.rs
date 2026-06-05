//! Private helper methods for Nimbus contract

use alloc::vec::Vec;
use alloy_primitives::Address;
use crate::storage::Nimbus;

impl Nimbus {
    #[inline(always)]
    pub(crate) fn msg_sender(&self) -> Address {
        #[cfg(test)]
        {
            crate::tests::MSG_SENDER.with(|s| *s.borrow())
        }
        #[cfg(not(test))]
        {
            stylus_sdk::msg::sender()
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
            stylus_sdk::block::timestamp()
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
}
