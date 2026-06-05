//! Dynamic Vault Model for Treasury Management
//!
//! Implements cascading liquidity buffer across Cash, Aave V3, and RWA T-Bills with
//! dynamic rebalancing based on 7-epoch moving average volume.

use alloc::vec::Vec;
use alloy_primitives::{Address, U256};

use crate::storage::Nimbus;
use crate::interfaces::{IErc20, IAavePool, IRwaToken};

impl Nimbus {
    /// Update the current epoch volume and recalculate the dynamic cash reserve ratio using a 7-epoch moving average.
    pub(crate) fn update_epoch_and_rebalance_ratio(&mut self, amount: U256) -> Result<(), Vec<u8>> {
        let current_vol = self.current_epoch_volume.get();
        self.current_epoch_volume.set(current_vol + amount);

        let current_time = U256::from(self.block_timestamp());
        let epoch_start = self.epoch_start_timestamp.get();
        
        // Epoch duration: 24 hours (86400 seconds)
        if current_time >= epoch_start + U256::from(86400) {
            let epoch_id = self.current_epoch_id.get();
            let epoch_vol = self.current_epoch_volume.get();
            
            // Store current epoch volume
            self.historical_epoch_volumes.insert(epoch_id, epoch_vol);

            // Compute Moving Average of the last 7 epochs (or fewer if we just started)
            let mut sum_vol = U256::ZERO;
            let start_idx = if epoch_id >= U256::from(6) {
                epoch_id - U256::from(6)
            } else {
                U256::ZERO
            };
            
            let mut count = U256::ZERO;
            let mut idx = start_idx;
            while idx <= epoch_id {
                sum_vol += self.historical_epoch_volumes.get(idx);
                count += U256::from(1);
                idx += U256::from(1);
            }
            
            let moving_average_volume = if count > U256::ZERO {
                sum_vol / count
            } else {
                U256::ZERO
            };

            // High volume threshold: 100,000 USDC (in 6 decimals: 100_000_000_000)
            // Low volume threshold: 10,000 USDC (in 6 decimals: 10_000_000_000)
            let high_threshold = U256::from(100_000_000_000u64);
            let low_threshold = U256::from(10_000_000_000u64);

            if moving_average_volume > high_threshold {
                self.target_cash_pct.set(U256::from(45));
            } else if moving_average_volume < low_threshold {
                self.target_cash_pct.set(U256::from(15));
            } else {
                self.target_cash_pct.set(U256::from(30));
            }

            // Reset epoch parameters
            self.epoch_start_timestamp.set(current_time);
            self.current_epoch_id.set(epoch_id + U256::from(1));
            self.current_epoch_volume.set(U256::ZERO);
        }

        Ok(())
    }

    /// Calculate the Fast-Path Liquidity Premium dynamically (Roadmap Fase 1-3).
    /// Inspired by the 2026 academic paper "Exploiting Liquidity Exhaustion Attacks in Intent-Based Cross-Chain Bridges"
    /// we implement dynamic congestion-based pricing to prevent liquidity exhaustion attacks.
    pub fn calculate_fast_path_premium(&self, amount: U256) -> Result<U256, Vec<u8>> {
        let phase = self.fast_path_phase.get();
        if phase == U256::from(1) {
            // Fase 1: Disabled (CCIP slow path only, zero premium)
            return Ok(U256::ZERO);
        } else if phase == U256::from(2) {
            // Fase 2: Enabled via internal Treasury capital, flat 0.05% premium
            let premium = amount * U256::from(5) / U256::from(10000);
            return Ok(premium);
        } else if phase == U256::from(3) {
            // Fase 3: Public LP with Dynamic Cap and Congestion-based Pricing
            let total = self.total_lp_liquidity.get();
            let utilized = self.utilized_lp_liquidity.get();
            if total == U256::ZERO {
                return Err(b"ZERO_POOL_LIQUIDITY".to_vec());
            }
            let new_utilized = utilized + amount;
            // Enforce Dynamic Pool Cap: reject if it exceeds total liquidity (prevents exhaustion attacks)
            if new_utilized > total {
                return Err(b"LP_POOL_EXHAUSTED_DYNAMIC_CAP".to_vec());
            }
            
            // Calculate utilization rate: U = (utilized * 10000) / total (in basis points)
            let u_bps = (new_utilized * U256::from(10000)) / total;
            
            // Dynamic premium rate: base 5 bps (0.05%) up to max 15 bps (0.15%)
            // rate_bps = 5 + (10 * u_bps / 10000)
            let rate_bps = U256::from(5) + (U256::from(10) * u_bps / U256::from(10000));
            
            let premium = amount * rate_bps / U256::from(10000);
            return Ok(premium);
        }
        
        Ok(U256::ZERO)
    }

    /// Allocate incoming deposit amount according to the Dynamic Vault Model:
    /// cash_pct% Cash (dynamic), remaining split 5:2 between Aave V3 and RWA T-Bills.
    /// The target_cash_pct is updated by the 7-epoch moving average rebalancer.
    pub(crate) fn allocate_reserves(&mut self, amount: U256) -> Result<(), Vec<u8>> {
        #[cfg(not(test))]
        {
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let this_address = stylus_sdk::contract::address();
            
            // Read dynamic cash percentage (default 30, range 15-45)
            let cash_pct = self.target_cash_pct.get();
            let non_cash_pct = U256::from(100) - cash_pct;
            
            // Split non-cash portion 5:2 between Aave and RWA (≈71.4% / 28.6%)
            // When cash=30 → Aave=50%, RWA=20% (original ratio preserved)
            // When cash=45 → Aave=39.3%, RWA=15.7%
            // When cash=15 → Aave=60.7%, RWA=24.3%
            let aave_share = amount * non_cash_pct * U256::from(5) / (U256::from(100) * U256::from(7));
            let rwa_share = amount * non_cash_pct * U256::from(2) / (U256::from(100) * U256::from(7));
            
            // 1. Supply to Aave Pool V3
            let aave_pool_addr = self.aave_pool.get();
            if aave_pool_addr != Address::ZERO && aave_share > U256::ZERO {
                let aave = IAavePool::new(aave_pool_addr);
                let success = erc20.approve(&mut *self, aave_pool_addr, aave_share)
                    .map_err(|e| e)?;
                if success {
                    aave.supply(&mut *self, stablecoin_address, aave_share, this_address, 0)
                        .unwrap_or(());
                }
            }
            
            // 2. Supply to Ondo USDY / BlackRock BUIDL
            let rwa_token_addr = self.rwa_token.get();
            if rwa_token_addr != Address::ZERO && rwa_share > U256::ZERO {
                // TODO: In mainnet deployment, add KYC allowlist verification/checking for RWA tokens (Ondo/BlackRock)
                let rwa = IRwaToken::new(rwa_token_addr);
                let success = erc20.approve(&mut *self, rwa_token_addr, rwa_share)
                    .map_err(|e| e)?;
                if success {
                    rwa.deposit(&mut *self, rwa_share)
                        .unwrap_or(U256::ZERO);
                }
            }
        }
        Ok(())
    }

    /// Ensures that the contract has at least `required_amount` of stablecoin.
    /// Uses a Cascading Liquidity Buffer (Roadmap Item 2):
    /// 1. Uses cash (USDC balance of the contract).
    /// 2. If insufficient, withdraws shortfall from Aave V3 (Instant Liquidity tier).
    /// 3. If still insufficient, redeems RWA T-Bills (Ondo USDY / BlackRock BUIDL) (Reserve tier).
    pub(crate) fn ensure_liquidity(&mut self, required_amount: U256) -> Result<(), Vec<u8>> {
        #[cfg(not(test))]
        {
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            
            let this_address = stylus_sdk::contract::address();
            let mut cash_balance = erc20.balance_of(&mut *self, this_address)
                .map_err(|e| e)?;
                
            if cash_balance >= required_amount {
                return Ok(());
            }
            
            let mut shortfall = required_amount - cash_balance;
            
            // Tier 2: Withdraw from Aave Pool V3
            let aave_pool_addr = self.aave_pool.get();
            if aave_pool_addr != Address::ZERO {
                let aave = IAavePool::new(aave_pool_addr);
                let withdrawn = aave.withdraw(&mut *self, stablecoin_address, shortfall, this_address)
                    .unwrap_or(U256::ZERO);
                
                cash_balance = erc20.balance_of(&mut *self, this_address)
                    .map_err(|e| e)?;
                if cash_balance >= required_amount {
                    return Ok(());
                }
                shortfall = required_amount - cash_balance;
            }
            
            // Tier 3: Redeem from RWA T-Bills (Ondo USDY / BlackRock BUIDL)
            let rwa_token_addr = self.rwa_token.get();
            if rwa_token_addr != Address::ZERO && shortfall > U256::ZERO {
                // TODO: In mainnet deployment, integrate Chainlink oracle price feeds for Ondo USDY / BUIDL NAV calculation
                let rwa = IRwaToken::new(rwa_token_addr);
                let _redeemed = rwa.redeem(&mut *self, shortfall, shortfall)
                    .unwrap_or(U256::ZERO);
                
                cash_balance = erc20.balance_of(&mut *self, this_address)
                    .map_err(|e| e)?;
                if cash_balance < required_amount {
                    return Err(b"INSUFFICIENT_TOTAL_LIQUIDITY_IN_VAULT_BUFFERS".to_vec());
                }
            } else if shortfall > U256::ZERO {
                return Err(b"INSUFFICIENT_CASH_AND_AAVE_LIQUIDITY".to_vec());
            }
        }
        Ok(())
    }

    pub fn total_assets(&mut self) -> Result<U256, Vec<u8>> {
        #[cfg(test)]
        {
            Ok(self.total_deposited_principal.get())
        }
        #[cfg(not(test))]
        {
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let this_address = stylus_sdk::contract::address();
            let cash = erc20.balance_of(&mut *self, this_address).unwrap_or(U256::ZERO);
            
            let a_token_addr = self.a_token.get();
            let aave_assets = if a_token_addr != Address::ZERO {
                let a_token = IErc20::new(a_token_addr);
                a_token.balance_of(&mut *self, this_address).unwrap_or(U256::ZERO)
            } else {
                U256::ZERO
            };
            
            let rwa_token_addr = self.rwa_token.get();
            let rwa_assets = if rwa_token_addr != Address::ZERO {
                let rwa = IErc20::new(rwa_token_addr);
                rwa.balance_of(&mut *self, this_address).unwrap_or(U256::ZERO)
            } else {
                U256::ZERO
            };
            
            Ok(cash + aave_assets + rwa_assets)
        }
    }

    pub fn claim_accumulated_yield(&mut self) -> Result<U256, Vec<u8>> {
        self.check_owner()?;
        let total = self.total_assets()?;
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
            let success = erc20.transfer(&mut *self, recipient, yield_amount)
                .map_err(|e| e)?;
            if !success {
                return Err(b"YIELD_TRANSFER_FAILED".to_vec());
            }
        }
        
        Ok(yield_amount)
    }
}
