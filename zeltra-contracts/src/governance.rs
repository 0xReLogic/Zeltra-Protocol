//! Governance, Administration, and Configuration methods for Zeltra contract

use crate::storage::Zeltra;
use alloc::vec::Vec;
use alloy_primitives::{keccak256, Address, FixedBytes};
use stylus_sdk::{abi::Bytes, alloy_primitives::U256};

impl Zeltra {
    // --- Contract Initialization ---

    pub fn _init(
        &mut self,
        owner: Address,
        stablecoin_addr: Address,
        fee_recipient_addr: Address,
    ) -> Result<(), Vec<u8>> {
        if self.owner.get() != Address::ZERO {
            return Err(b"ALREADY_INITIALIZED".to_vec());
        }
        if owner == Address::ZERO {
            return Err(b"INVALID_OWNER".to_vec());
        }
        self.owner.set(owner);
        self.paused.set(false);
        let default_token = if stablecoin_addr == Address::ZERO {
            Address::repeat_byte(0xee)
        } else {
            stablecoin_addr
        };
        self.stablecoin.set(default_token);
        self.asset_tokens.setter(U256::from(1)).set(default_token);
        if self.asset_tokens.get(U256::ZERO) == Address::ZERO {
            self.asset_tokens.setter(U256::ZERO).set(default_token);
            self.weth9_address.set(default_token);
        }
        self.fee_recipient.set(fee_recipient_addr);
        self.epoch_start_timestamp
            .set(U256::from(self.block_timestamp()));
        self.generation_epoch_id.setter(U256::ZERO).set(U256::ZERO);
        Ok(())
    }

    pub fn _init_multi_asset(
        &mut self,
        owner: Address,
        weth9_addr: Address,
        usdc_addr: Address,
        fee_recipient_addr: Address,
    ) -> Result<(), Vec<u8>> {
        self._init(owner, usdc_addr, fee_recipient_addr)?;
        self.weth9_address.set(weth9_addr);
        self.asset_tokens.setter(U256::ZERO).set(weth9_addr);
        Ok(())
    }

    // --- Multi-Asset & Token Configuration ---

    pub fn _set_asset_token(&mut self, asset_id: U256, token_addr: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if token_addr == Address::ZERO {
            return Err(b"INVALID_TOKEN_ADDRESS".to_vec());
        }
        self.asset_tokens.setter(asset_id).set(token_addr);
        if asset_id == U256::ZERO {
            self.weth9_address.set(token_addr);
        } else if asset_id == U256::from(1) {
            self.stablecoin.set(token_addr);
        }
        Ok(())
    }

    pub fn _set_weth9_address(&mut self, weth9: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if weth9 == Address::ZERO {
            return Err(b"INVALID_WETH9_ADDRESS".to_vec());
        }
        self.weth9_address.set(weth9);
        self.asset_tokens.setter(U256::ZERO).set(weth9);
        Ok(())
    }

    // --- Two-Step Governance (Issue 10) ---

    pub fn _propose_owner(&mut self, new_owner: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if new_owner == Address::ZERO {
            return Err(b"INVALID_NEW_OWNER".to_vec());
        }
        self.pending_owner.set(new_owner);
        Ok(())
    }

    pub fn _claim_ownership(&mut self) -> Result<(), Vec<u8>> {
        let pending = self.pending_owner.get();
        if pending != self.msg_sender() {
            return Err(b"NOT_PENDING_OWNER".to_vec());
        }
        self.owner.set(pending);
        self.pending_owner.set(Address::ZERO);
        Ok(())
    }

    // --- Timelocked Admin Parameter Changes (Issue 10) ---

    pub fn _propose_fee_recipient(&mut self, recipient: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if recipient == Address::ZERO {
            return Err(b"INVALID_RECIPIENT".to_vec());
        }
        self.proposed_fee_recipient.set(recipient);
        self.fee_recipient_eta
            .set(U256::from(self.block_timestamp() + 86400));
        Ok(())
    }

    pub fn _execute_fee_recipient(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let eta = self.fee_recipient_eta.get();
        if eta == U256::ZERO {
            return Err(b"NO_PROPOSAL_ACTIVE".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        if current_time < eta {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        let recipient = self.proposed_fee_recipient.get();
        self.fee_recipient.set(recipient);
        self.fee_recipient_eta.set(U256::ZERO);
        Ok(())
    }

    // --- Dynamic Deposit Fee Configuration (docs/todos/fee-policy-zero-deposit.md) ---

    pub fn _propose_deposit_fee_bps(&mut self, new_fee_bps: U256) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if new_fee_bps > U256::from(50) {
            return Err(b"DEPOSIT_FEE_EXCEEDS_MAX".to_vec());
        }
        self.proposed_deposit_fee_bps.set(new_fee_bps);
        self.deposit_fee_eta
            .set(U256::from(self.block_timestamp() + 86400));
        Ok(())
    }

    pub fn _execute_deposit_fee_bps(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let eta = self.deposit_fee_eta.get();
        if eta == U256::ZERO {
            return Err(b"NO_PROPOSAL_ACTIVE".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        if current_time < eta {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        let old_fee_bps = self.get_deposit_fee_bps();
        let new_fee_bps = self.proposed_deposit_fee_bps.get();
        self.deposit_fee_bps.set(new_fee_bps);
        self.deposit_fee_initialized.set(true);
        self.deposit_fee_eta.set(U256::ZERO);

        crate::events::emit_event(crate::events::DepositFeeConfigUpdated {
            old_fee_bps,
            new_fee_bps,
        });

        Ok(())
    }

    pub fn _set_deposit_fee_bps(&mut self, new_fee_bps: U256) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if new_fee_bps > U256::from(50) {
            return Err(b"DEPOSIT_FEE_EXCEEDS_MAX".to_vec());
        }
        let old_fee_bps = self.get_deposit_fee_bps();
        self.deposit_fee_bps.set(new_fee_bps);
        self.deposit_fee_initialized.set(true);

        crate::events::emit_event(crate::events::DepositFeeConfigUpdated {
            old_fee_bps,
            new_fee_bps,
        });

        Ok(())
    }

    // --- Execution Fee Configuration & Claims ---

    pub fn _set_execution_fee_recipient(&mut self, recipient: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if recipient == Address::ZERO {
            return Err(b"INVALID_RECIPIENT".to_vec());
        }
        self.execution_fee_recipient.set(recipient);
        Ok(())
    }

    pub fn _claim_execution_fees(&mut self, asset_id: u8, amount: U256) -> Result<bool, Vec<u8>> {
        let recipient = self.execution_fee_recipient.get();
        if recipient == Address::ZERO {
            return Err(b"NO_RECIPIENT_SET".to_vec());
        }
        let caller = self.msg_sender();
        if caller != recipient && caller != self.owner.get() {
            return Err(b"NOT_AUTHORIZED".to_vec());
        }
        let asset_key = U256::from(asset_id);
        let accrued = self.accrued_execution_fee_liabilities.get(asset_key);
        if amount > accrued {
            return Err(b"INSUFFICIENT_ACCUMULATED_FEES".to_vec());
        }
        if amount == U256::ZERO {
            return Err(b"ZERO_AMOUNT".to_vec());
        }

        let token_addr = self.asset_tokens.get(asset_key);
        if token_addr == Address::ZERO {
            return Err(b"ASSET_NOT_CONFIGURED".to_vec());
        }

        // Scaling: Gwei * 10^9 = Wei for WETH, raw for USDC
        let raw_transfer_amount = if asset_id == 0 {
            amount
                .checked_mul(U256::from(1_000_000_000u64))
                .ok_or_else(|| b"OVERFLOW".to_vec())?
        } else {
            amount
        };

        // Transfer token to recipient
        self.asset_safe_transfer(token_addr, recipient, raw_transfer_amount)?;

        // Deduct liability
        self.accrued_execution_fee_liabilities
            .setter(asset_key)
            .set(accrued - amount);

        // Emit FeeClaim event
        crate::events::emit_event(crate::events::FeeClaim {
            recipient,
            amount,
            fee_type: keccak256(b"EXECUTION_FEE"),
        });

        // Enforce solvency invariant
        self._check_asset_solvency_invariant(asset_id)?;

        Ok(true)
    }

    // --- Circuit Breaker (Pause/Unpause) & Clean Roots ---

    pub fn _pause(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.paused.set(true);
        Ok(())
    }

    pub fn _unpause(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.paused.set(false);
        Ok(())
    }

    pub fn _register_clean_root(&mut self, root: FixedBytes<32>) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        self.check_owner()?;
        let current_time = U256::from(self.block_timestamp());
        self.clean_association_roots.insert(root, current_time);
        Ok(())
    }

    // --- CCIP Configuration ---

    pub fn _set_ccip_router(&mut self, router: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if router == Address::ZERO {
            return Err(b"INVALID_ROUTER_ADDRESS".to_vec());
        }
        self.ccip_router.set(router);
        Ok(())
    }

    pub fn _set_ccip_sender_allowlist(
        &mut self,
        source_chain_selector: u64,
        sender: Bytes,
        allowed: bool,
    ) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if source_chain_selector == 0 {
            return Err(b"INVALID_SOURCE_CHAIN_SELECTOR".to_vec());
        }
        if sender.is_empty() {
            return Err(b"EMPTY_CCIP_SENDER".to_vec());
        }
        let key = self.ccip_allowlist_key(source_chain_selector, &sender);
        self.ccip_allowed_senders.insert(key, allowed);
        Ok(())
    }

    // --- Epoch Rotation ---

    pub fn _rotate_epoch(&mut self) -> Result<u32, Vec<u8>> {
        self.check_not_paused()?;
        self.check_owner()?;

        let old_epoch = self.current_epoch.get().to::<u32>();
        let new_epoch = old_epoch
            .checked_add(1)
            .ok_or_else(|| b"EPOCH_OVERFLOW".to_vec())?;
        let recycle_slot = (new_epoch % 2) as u64;

        self.current_epoch.set(U256::from(new_epoch));
        let now = self.block_timestamp();
        self.epoch_start_timestamp.set(U256::from(now));
        self.generation_epoch_id
            .setter(U256::from(recycle_slot))
            .set(U256::from(new_epoch));

        crate::events::emit_event(crate::events::EpochRotated {
            new_epoch,
            timestamp: now,
        });

        Ok(new_epoch)
    }
}
