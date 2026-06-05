# Additional Threat Mitigation Strategies: Nimbus Contracts

**Document Date:** June 5, 2026  
**Scope:** Prevention strategies for 10 additional security threats identified during audit  
**Focus:** Oracle manipulation, timestamp manipulation, governance security, cross-chain finality, gas griefing, front-running, ZK circuit security, MEV protection, privacy preservation, RWA compliance

---

## Overview

This document provides comprehensive mitigation strategies for 10 additional security threats identified during the Nimbus Contracts security audit. While these threats were not explicitly documented as vulnerabilities in the main audit report, they represent important design considerations and potential attack vectors that should be addressed to ensure comprehensive security.

---

## 1. Oracle Manipulation Prevention

**Severity:** HIGH  
**Category:** Price Oracle Manipulation (OWASP SC03)  
**Current Risk:** Contract uses Aave/RWA price feeds implicitly without explicit oracle validation

### Research Sources
- OWASP Smart Contract Top 10 2026 - SC03: Price Oracle Manipulation
- Smart Contract Hacking: Oracle Manipulation Attacks (2026)
- Chainlink Oracle Security Research
- Alchemy Smart Contract Security Best Practices

### Attack Scenario
1. Attacker manipulates Aave or RWA price feeds through flash loan attacks
2. Vault `total_assets()` calculation returns incorrect value
3. Yield claims or liquidity decisions based on incorrect prices
4. Protocol loses funds or makes incorrect economic decisions

### Mitigation Strategies

#### A. Use Decentralized Oracle Networks
```rust
// RECOMMENDED: Integrate Chainlink Price Feeds
// Add to interfaces.rs:
sol_interface! {
    interface IChainlinkPriceFeed {
        function latestRoundData() external view returns (
            uint80 roundId,
            int256 answer,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        );
    }
}

// Add to vault.rs:
pub(crate) fn get_safe_price(&self, feed_address: Address) -> Result<U256, Vec<u8>> {
    #[cfg(not(test))]
    {
        let feed = IChainlinkPriceFeed::new(feed_address);
        let (_, price, _, updated_at, _) = feed.latest_round_data(&mut *self)
            .map_err(|_| b"ORACLE_CALL_FAILED".to_vec())?;
        
        // Validate price is positive
        if price <= 0 {
            return Err(b"INVALID_PRICE".to_vec());
        }
        
        // Validate freshness (max 1 hour old)
        let current_time = U256::from(self.block_timestamp());
        let update_time = U256::from(updated_at);
        if current_time > update_time + U256::from(3600) {
            return Err(b"STALE_ORACLE_DATA".to_vec());
        }
        
        // Validate price is within reasonable bounds (sanity check)
        let price_u256 = U256::from(price as u64);
        if price_u256 < U256::from(100_000) || price_u256 > U256::from(10_000_000_000u64) {
            return Err(b"PRICE_OUT_OF_BOUNDS".to_vec());
        }
        
        Ok(price_u256)
    }
    #[cfg(test)]
    {
        Ok(U256::from(1_000_000)) // Mock price for testing
    }
}
```

#### B. Implement Multi-Oracle Check
```rust
// Add to vault.rs:
pub(crate) fn get_cross_chain_price(&self, feeds: Vec<Address>) -> Result<U256, Vec<u8>> {
    let mut prices = Vec::new();
    
    for feed in feeds {
        let price = self.get_safe_price(feed)?;
        prices.push(price);
    }
    
    // Calculate median price
    prices.sort();
    let median = if prices.len() % 2 == 0 {
        (prices[prices.len() / 2 - 1] + prices[prices.len() / 2]) / U256::from(2)
    } else {
        prices[prices.len() / 2]
    };
    
    // Validate all prices are within 5% of median
    for price in &prices {
        let diff = if *price > median { *price - median } else { median - *price };
        let threshold = median * U256::from(5) / U256::from(100);
        if diff > threshold {
            return Err(b"ORACLE_PRICE_DIVERGENCE".to_vec());
        }
    }
    
    Ok(median)
}
```

#### C. Add Circuit Breakers
```rust
// Add to storage.rs:
uint256 oracle_deviations_count;
uint256 last_circuit_breaker_trigger;

// Add to vault.rs:
pub(crate) fn check_oracle_health(&mut self) -> Result<(), Vec<u8>> {
    let deviation_count = self.oracle_deviations_count.get();
    
    // If 3+ deviations in 1 hour, pause protocol
    if deviation_count >= U256::from(3) {
        let last_trigger = self.last_circuit_breaker_trigger.get();
        let current_time = U256::from(self.block_timestamp());
        
        if current_time < last_trigger + U256::from(3600) {
            self.paused.set(true);
            return Err(b"CIRCUIT_BREAKER_TRIGGERED".to_vec());
        }
        
        // Reset counter if 1 hour passed
        self.oracle_deviations_count.set(U256::ZERO);
    }
    
    Ok(())
}
```

### Implementation Priority
- **HIGH:** Integrate Chainlink Price Feeds with validation
- **MEDIUM:** Implement multi-oracle checks for critical operations
- **LOW:** Add circuit breakers and monitoring

---

## 2. Timestamp Manipulation Prevention

**Severity:** MEDIUM  
**Category:** Block Variable Manipulation (OWASP SCWE-031)  
**Current Risk:** `block.timestamp` used for 24-hour timelock in `claim_refund()`

### Research Sources
- OWASP Smart Contract Weakness Enumeration - SCWE-031
- Alchemy Smart Contract Security Best Practices
- Smart Contract Hacking: Time Manipulation

### Attack Scenario
1. Miner/validator can manipulate `block.timestamp` by ±15 seconds
2. Attacker exploits this to bypass timelock checks earlier than intended
3. While not critical for 24-hour timelocks, it's a best practice violation

### Mitigation Strategies

#### A. Use Block Numbers Instead of Timestamps
```rust
// Modify deposit.rs:
// Current: Uses block.timestamp
if current_time < deposit_time + U256::from(86400) {
    return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
}

// RECOMMENDED: Use block numbers (12 seconds per block on Ethereum)
// 24 hours = 7200 blocks
if self.block_number() < deposit_block + U256::from(7200) {
    return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
}

// Add to helpers.rs:
#[inline(always)]
pub(crate) fn block_number(&self) -> U256 {
    #[cfg(test)]
    {
        // Mock for testing
        U256::from(1000)
    }
    #[cfg(not(test))]
    {
        stylus_sdk::block::number()
    }
}
```

#### B. Add Safety Margin
```rust
// If must use timestamps, add safety margin
// Current: 86400 seconds (24 hours)
// RECOMMENDED: 86385 seconds (24 hours - 15 seconds margin)
if current_time < deposit_time + U256::from(86385) {
    return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
}
```

#### C. Use Block Timestamp for Rough Estimates Only
```rust
// Acceptable uses:
// - "Has approximately 24 hours passed?" (current implementation)
// - Epoch tracking (current implementation)

// Unacceptable uses:
// - Precise timing for lotteries/gambling
// - Random number generation
// - Exact deadline enforcement where seconds matter
```

### Implementation Priority
- **LOW:** Current usage is acceptable (rough time estimate)
- **OPTIONAL:** Consider block numbers for more precise timing

---

## 3. Compliance Root Centralization Prevention

**Severity:** HIGH  
**Category:** Access Control / Governance  
**Current Risk:** Owner can register any compliance root without governance oversight

### Research Sources
- Trail of Bits: Maturing Smart Contracts Beyond Private Key Risk
- Octane Security: Upgradeable Smart Contracts Security
- SEAL Frameworks: Secure Multisig Best Practices
- OWASP Smart Contract Top 10 2026 - Governance-Specific Attack Vectors

### Attack Scenario
1. Owner private key is compromised
2. Attacker registers malicious compliance root
3. Attacker can bypass compliance checks entirely
4. Protocol loses all compliance guarantees

### Mitigation Strategies

#### A. Implement Timelock for Root Registration
```rust
// Add to storage.rs:
mapping(bytes32 => uint256) pending_roots; // root => timestamp
uint256 root_registration_timelock; // e.g., 48 hours

// Modify lib.rs:
pub fn propose_compliance_root(&mut self, root: FixedBytes<32>) -> Result<(), Vec<u8>> {
    self.check_owner()?;
    self.check_not_paused()?;
    
    let current_time = U256::from(self.block_timestamp());
    self.pending_roots.insert(root, current_time);
    
    Ok(())
}

pub fn execute_compliance_root(&mut self, root: FixedBytes<32>) -> Result<(), Vec<u8>> {
    self.check_owner()?;
    self.check_not_paused()?;
    
    let proposal_time = self.pending_roots.get(root);
    if proposal_time == U256::ZERO {
        return Err(b"NO_PENDING_ROOT".to_vec());
    }
    
    let current_time = U256::from(self.block_timestamp());
    let timelock = self.root_registration_timelock.get();
    
    if current_time < proposal_time + timelock {
        return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
    }
    
    self.clean_association_roots.insert(root, true);
    self.pending_roots.insert(root, U256::ZERO);
    
    Ok(())
}

pub fn cancel_compliance_root(&mut self, root: FixedBytes<32>) -> Result<(), Vec<u8>> {
    self.check_owner()?;
    self.pending_roots.insert(root, U256::ZERO);
    Ok(())
}
```

#### B. Implement Multi-Sig for Owner
```rust
// RECOMMENDED: Use Safe (Gnosis Safe) multi-sig as owner
// Minimum 3/5 signers for critical operations
// Geographic distribution of keys
// Regular key rotation and backup procedures

// Implementation:
// 1. Deploy Safe multi-sig wallet
// 2. Transfer ownership to Safe address
// 3. Set up governance procedures
// 4. Document key management policies
```

#### C. Add Governance Role Separation
```rust
// Add to storage.rs:
address governance; // Separate from owner
address compliance_admin; // Can propose roots, needs governance approval

// Modify lib.rs:
pub fn set_governance(&mut self, gov: Address) -> Result<(), Vec<u8>> {
    self.check_owner()?;
    // Require timelock for governance change
    Ok(())
}

pub fn propose_compliance_root_gov(&mut self, root: FixedBytes<32>) -> Result<(), Vec<u8>> {
    // Compliance admin can propose
    // Governance must approve after timelock
    Ok(())
}
```

### Implementation Priority
- **HIGH:** Implement timelock for root registration
- **HIGH:** Transfer ownership to multi-sig
- **MEDIUM:** Add governance role separation

---

## 4. RWA Token Risks Prevention

**Severity:** HIGH  
**Category:** Regulatory Compliance / Integration Risks  
**Current Risk:** TODO comment mentions KYC allowlist but not implemented

### Research Sources
- Safeheron: How to Buy RWA Tokens Safely in 2025
- RWA.io: Using Smart Contracts for Secure RWA Transactions
- Ment.tech: Enterprise Grade Tokenization & RWA Compliance Services
- Three Sigma: Real World Asset Tokenization Explained

### Attack Scenario
1. Contract attempts to deposit to RWA token (Ondo USDY / BlackRock BUIDL)
2. RWA token has KYC/AML restrictions
3. Contract is not on allowlist
4. Transaction fails or funds get stuck
5. Accounting mismatch

### Mitigation Strategies

#### A. Implement KYC Allowlist Check
```rust
// Add to storage.rs:
mapping(address => bool) rwa_allowed_contracts;
mapping(address => bool) kyc_whitelist;

// Modify vault.rs:
pub(crate) fn allocate_reserves(&mut self, amount: U256) -> Result<(), Vec<u8>> {
    #[cfg(not(test))]
    {
        let stablecoin_address = self.stablecoin.get();
        let erc20 = IErc20::new(stablecoin_address);
        let this_address = stylus_sdk::contract::address();
        
        let cash_pct = self.target_cash_pct.get();
        let non_cash_pct = U256::from(100) - cash_pct;
        
        let aave_share = amount * non_cash_pct * U256::from(5) / (U256::from(100) * U256::from(7));
        let rwa_share = amount * non_cash_pct * U256::from(2) / (U256::from(100) * U256::from(7));
        
        // Aave supply (existing code)
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
        
        // RWA supply with KYC check
        let rwa_token_addr = self.rwa_token.get();
        if rwa_token_addr != Address::ZERO && rwa_share > U256::ZERO {
            // NEW: Check if contract is KYC-approved
            if !self.rwa_allowed_contracts.get(this_address) {
                return Err(b"CONTRACT_NOT_KYC_APPROVED".to_vec());
            }
            
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

// Add to lib.rs:
pub fn set_rwa_allowed_contract(&mut self, contract: Address, allowed: bool) -> Result<(), Vec<u8>> {
    self.check_owner()?;
    self.rwa_allowed_contracts.insert(contract, allowed);
    Ok(())
}
```

#### B. Implement ERC-3643 Compliance Standard
```rust
// RECOMMENDED: Use ERC-3643 (T-REX) for RWA tokens
// Provides built-in compliance features:
// - Identity registry
// - Transfer restrictions
// - Jurisdiction checks
// - Accredited investor verification

// Integration example:
sol_interface! {
    interface IERC3643Token {
        function isTransferAllowed(address _from, address _to, uint256 _amount) external view returns (bool);
    }
}

// Add check before RWA operations:
let token = IERC3643Token::new(rwa_token_addr);
if !token.is_transfer_allowed(&mut *self, this_address, this_address, rwa_share) {
    return Err(b"COMPLIANCE_CHECK_FAILED".to_vec());
}
```

#### C. Add Circuit Breaker for RWA Operations
```rust
// Add to storage.rs:
bool rwa_operations_paused;
uint256 last_rwa_failure;

// Modify vault.rs:
pub(crate) fn safe_rwa_deposit(&mut self, amount: U256) -> Result<(), Vec<u8>> {
    if self.rwa_operations_paused.get() {
        return Err(b"RWA_OPERATIONS_PAUSED".to_vec());
    }
    
    let rwa_token_addr = self.rwa_token.get();
    let rwa = IRwaToken::new(rwa_token_addr);
    
    match rwa.deposit(&mut *self, amount) {
        Ok(result) => Ok(result),
        Err(_) => {
            // Pause RWA operations on failure
            self.rwa_operations_paused.set(true);
            self.last_rwa_failure.set(U256::from(self.block_timestamp()));
            Err(b"RWA_DEPOSIT_FAILED_PAUSED".to_vec())
        }
    }
}

// Add to lib.rs:
pub fn unpause_rwa_operations(&mut self) -> Result<(), Vec<u8>> {
    self.check_owner()?;
    self.rwa_operations_paused.set(false);
    Ok(())
}
```

### Implementation Priority
- **HIGH:** Implement KYC allowlist check before mainnet
- **MEDIUM:** Consider ERC-3643 integration for institutional use
- **LOW:** Add circuit breakers for RWA operations

---

## 5. Cross-Chain Finality Prevention

**Severity:** MEDIUM  
**Category:** Cross-Chain Security / Finality  
**Current Risk:** CCIP integration has fallback but no explicit finality check

### Research Sources
- Chainlink CCIP Documentation
- Symbiosis Finance: DeFi in 2025-2026 Technical Changes
- Cryptobriefing: Re switches to Chainlink CCIP
- CoinMarketCap: Kraken switches to Chainlink CCIP after $292M Kelp bridge exploit

### Attack Scenario
1. Source chain reorgs (possible on optimistic rollups)
2. CCIP message already processed on destination chain
3. Double-spend across chains
4. Funds drained

### Mitigation Strategies

#### A. Add Finality Check
```rust
// Add to storage.rs:
mapping(uint256 => uint256) processed_ccip_messages; // message_id => block_number

// Modify spend.rs:
pub fn ccip_receive(
    &mut self,
    message_id: FixedBytes<32>,
    source_chain_selector: u64,
    _sender: Vec<u8>,
    payload: Vec<u8>,
) -> Result<(), Vec<u8>> {
    if payload.len() != 648 {
        return Err(b"INVALID_CCIP_PAYLOAD_LENGTH".to_vec());
    }
    
    // NEW: Check for replay attack
    let msg_id_hash = FixedBytes::from_slice(&message_id.0[..32]);
    if self.processed_ccip_messages.get(msg_id_hash) != U256::ZERO {
        return Err(b"CCIP_MESSAGE_ALREADY_PROCESSED".to_vec());
    }
    
    // NEW: Add finality delay for optimistic rollups
    // Arbitrum: ~1 hour finality
    // Optimism: ~1 hour finality
    // Base: ~1 hour finality
    let current_block = self.block_number();
    let min_finality_blocks = U256::from(300); // ~1 hour on L2
    
    // Store message with block number
    self.processed_ccip_messages.insert(msg_id_hash, current_block);
    
    // Rest of existing code...
    let mut nullifier = [0u8; 32];
    nullifier.copy_from_slice(&payload[0..32]);
    
    let alpha_neg_bytes = payload[32..160].to_vec();
    let hm_bytes = payload[160..288].to_vec();
    let pk_iss_bytes = payload[288..544].to_vec();
    
    let polymarket_ctf = Address::from_slice(&payload[544..564]);
    let collateral_token = Address::from_slice(&payload[564..584]);
    
    let mut condition_id = [0u8; 32];
    condition_id.copy_from_slice(&payload[584..616]);
    
    let amount = U256::from_be_slice(&payload[616..648]);
    
    let success = self.spend_and_buy_shares(
        nullifier.into(),
        alpha_neg_bytes,
        hm_bytes,
        pk_iss_bytes,
        polymarket_ctf,
        collateral_token,
        condition_id.into(),
        amount,
    )?;
    
    if !success {
        return Err(b"CCIP_EXECUTION_FAILED".to_vec());
    }
    
    Ok(())
}
```

#### B. Implement Cross-Chain Rate Limiting
```rust
// Add to storage.rs:
uint256 ccip_rate_limit_per_hour;
mapping(uint256 => uint256) ccip_hourly_volume; // hour => volume
uint256 last_ccip_hour;

// Modify ccip_receive:
pub fn ccip_receive(&mut self, ...) -> Result<(), Vec<u8>> {
    // Check rate limit
    let current_hour = U256::from(self.block_timestamp()) / U256::from(3600);
    
    if current_hour != self.last_ccip_hour.get() {
        self.ccip_hourly_volume.set(U256::ZERO);
        self.last_ccip_hour.set(current_hour);
    }
    
    let current_volume = self.ccip_hourly_volume.get();
    let rate_limit = self.ccip_rate_limit_per_hour.get();
    
    if current_volume + amount > rate_limit {
        return Err(b"CCIP_RATE_LIMIT_EXCEEDED".to_vec());
    }
    
    self.ccip_hourly_volume.set(current_volume + amount);
    
    // Rest of existing code...
}
```

#### C. Add Chain-Specific Finality Configurations
```rust
// Add to storage.rs:
mapping(uint256 => uint256) chain_finality_blocks; // chain_selector => blocks

// Add to lib.rs:
pub fn set_chain_finality(&mut self, chain_selector: u64, blocks: U256) -> Result<(), Vec<u8>> {
    self.check_owner()?;
    self.chain_finality_blocks.insert(U256::from(chain_selector), blocks);
    Ok(())
}

// Default configurations in init():
// Ethereum L1: 0 blocks (instant finality)
// Arbitrum: 300 blocks (~1 hour)
// Optimism: 300 blocks (~1 hour)
// Base: 300 blocks (~1 hour)
// Polygon: 128 blocks (~10 minutes)
```

### Implementation Priority
- **HIGH:** Add replay protection and finality check
- **MEDIUM:** Implement rate limiting
- **LOW:** Add chain-specific configurations

---

## 6. Gas Griefing Prevention

**Severity:** MEDIUM  
**Category:** Denial of Service (OWASP SC10)  
**Current Risk:** External calls to Aave/RWA/Polymarket have no gas limits

### Research Sources
- Smart Contract Hacking: DoS Attacks
- OWASP Smart Contract Top 10 2026 - SC10 Denial of Service
- KPMG AADAPT: Gas Griefing Tactics
- Alchemy: Solidity Gas Optimization

### Attack Scenario
1. Attacker supplies minimal gas for transaction
2. Outer function succeeds (state updated)
3. Inner external call fails due to gas starvation (63/64 rule)
4. Transaction marked as executed but sub-call fails silently
5. State inconsistency

### Mitigation Strategies

#### A. Add Gas Limits to External Calls
```rust
// Stylus doesn't support direct gas limits like Solidity
// But we can use try-catch pattern with proper error handling

// Modify vault.rs:
pub(crate) fn allocate_reserves(&mut self, amount: U256) -> Result<(), Vec<u8>> {
    #[cfg(not(test))]
    {
        let stablecoin_address = self.stablecoin.get();
        let erc20 = IErc20::new(stablecoin_address);
        let this_address = stylus_sdk::contract::address();
        
        let cash_pct = self.target_cash_pct.get();
        let non_cash_pct = U256::from(100) - cash_pct;
        
        let aave_share = amount * non_cash_pct * U256::from(5) / (U256::from(100) * U256::from(7));
        let rwa_share = amount * non_cash_pct * U256::from(2) / (U256::from(100) * U256::from(7));
        
        // Aave supply with explicit error handling
        let aave_pool_addr = self.aave_pool.get();
        if aave_pool_addr != Address::ZERO && aave_share > U256::ZERO {
            let aave = IAavePool::new(aave_pool_addr);
            
            // Approve first
            let approve_success = erc20.approve(&mut *self, aave_pool_addr, aave_share)
                .map_err(|e| b"AAVE_APPROVE_FAILED".to_vec())?;
            
            if !approve_success {
                return Err(b"AAVE_APPROVE_FAILED".to_vec());
            }
            
            // Supply with explicit error handling
            match aave.supply(&mut *self, stablecoin_address, aave_share, this_address, 0) {
                Ok(_) => {},
                Err(_) => {
                    // Log failure but don't revert entire transaction
                    // Consider implementing partial allocation
                    return Err(b"AAVE_SUPPLY_FAILED".to_vec());
                }
            }
        }
        
        // RWA supply with explicit error handling
        let rwa_token_addr = self.rwa_token.get();
        if rwa_token_addr != Address::ZERO && rwa_share > U256::ZERO {
            if !self.rwa_allowed_contracts.get(this_address) {
                return Err(b"CONTRACT_NOT_KYC_APPROVED".to_vec());
            }
            
            let rwa = IRwaToken::new(rwa_token_addr);
            
            let approve_success = erc20.approve(&mut *self, rwa_token_addr, rwa_share)
                .map_err(|e| b"RWA_APPROVE_FAILED".to_vec())?;
            
            if !approve_success {
                return Err(b"RWA_APPROVE_FAILED".to_vec());
            }
            
            match rwa.deposit(&mut *self, rwa_share) {
                Ok(_) => {},
                Err(_) => {
                    return Err(b"RWA_DEPOSIT_FAILED".to_vec());
                }
            }
        }
    }
    Ok(())
}
```

#### B. Implement Checks-Effects-Interactions Pattern
```rust
// CRITICAL: All state updates must happen BEFORE external calls
// This is already identified as a critical vulnerability in main audit
// Fix this first before implementing gas limits

// Correct pattern:
pub fn deposit(&mut self, ...) -> Result<(), Vec<u8>> {
    // 1. CHECKS
    self.check_not_paused()?;
    // ... validation ...
    
    // 2. EFFECTS (update state)
    self.session_client.insert(sid, client);
    self.session_amount.insert(sid, net_amount);
    // ... all state updates ...
    
    // 3. INTERACTIONS (external calls)
    #[cfg(not(test))]
    {
        // Token transfers
        // External calls
    }
    
    Ok(())
}
```

#### C. Add Circuit Breaker for Failed Operations
```rust
// Add to storage.rs:
uint256 failed_allocation_count;
uint256 last_allocation_failure;

// Modify vault.rs:
pub(crate) fn safe_allocate_reserves(&mut self, amount: U256) -> Result<(), Vec<u8>> {
    let failure_count = self.failed_allocation_count.get();
    
    // If 5+ failures in 1 hour, pause allocations
    if failure_count >= U256::from(5) {
        let last_failure = self.last_allocation_failure.get();
        let current_time = U256::from(self.block_timestamp());
        
        if current_time < last_failure + U256::from(3600) {
            return Err(b"ALLOCATION_CIRCUIT_BREAKER".to_vec());
        }
        
        self.failed_allocation_count.set(U256::ZERO);
    }
    
    match self.allocate_reserves(amount) {
        Ok(_) => Ok(()),
        Err(e) => {
            self.failed_allocation_count.set(failure_count + U256::from(1));
            self.last_allocation_failure.set(U256::from(self.block_timestamp()));
            Err(e)
        }
    }
}
```

### Implementation Priority
- **CRITICAL:** Fix state consistency violations first (main audit)
- **HIGH:** Add explicit error handling to external calls
- **MEDIUM:** Implement circuit breakers

---

## 7. Front-Running Prevention

**Severity:** MEDIUM  
**Category:** Front-Running & MEV (OWASP Alternate Top 15)  
**Current Risk:** Deposit/reveal/spend operations can be front-run

### Research Sources
- Smart Contract Hacking: Front-Running Attacks (2026)
- OWASP Smart Contract Top 10 2026 - Front-Running & MEV
- Speedrun Ethereum: Front-Running & MEV Mitigation
- Hacken: Front-Running Mitigation

### Attack Scenario
1. User submits deposit transaction
2. Attacker sees transaction in mempool
3. Attacker submits higher gas transaction to steal session
4. Attacker gets the deposit/reveal/spend first
5. User's transaction fails

### Mitigation Strategies

#### A. Implement Commit-Reveal Scheme for Sensitive Operations
```rust
// Add to storage.rs:
mapping(bytes32 => Commitment) commitments;

struct Commitment {
    address committer;
    uint256 timestamp;
    bool revealed;
}

// Modify deposit.rs:
pub fn commit_deposit(&mut self, commitment: FixedBytes<32>) -> Result<(), Vec<u8>> {
    self.check_not_paused()?;
    
    let client = self.msg_sender();
    let current_time = U256::from(self.block_timestamp());
    
    let comm = Commitment {
        committer: client,
        timestamp: current_time,
        revealed: false,
    };
    
    // Store commitment (hash of actual parameters)
    // commitment = keccak256(sid, amount, secret)
    self.commitments.insert(commitment, comm);
    
    Ok(())
}

pub fn reveal_and_deposit(
    &mut self,
    sid: FixedBytes<32>,
    com_k_bytes: Vec<u8>,
    amount: U256,
    secret: FixedBytes<32>,
) -> Result<(), Vec<u8>> {
    self.check_not_paused()?;
    
    let client = self.msg_sender();
    
    // Reconstruct commitment hash
    let mut commitment_data = Vec::new();
    commitment_data.extend_from_slice(&sid.0);
    commitment_data.extend_from_slice(&amount.to_be_bytes::<32>());
    commitment_data.extend_from_slice(&secret.0);
    
    // In production, use proper keccak256
    let commitment_hash = FixedBytes::from_slice(&commitment_data[..32]);
    
    let comm = self.commitments.get(commitment_hash);
    if comm.committer == Address::ZERO {
        return Err(b"NO_COMMITMENT_FOUND".to_vec());
    }
    
    if comm.committer != client {
        return Err(b"NOT_COMMITTER".to_vec());
    }
    
    if comm.revealed {
        return Err(b"ALREADY_REVEALED".to_vec());
    }
    
    // Check timelock (e.g., 1 hour between commit and reveal)
    let current_time = U256::from(self.block_timestamp());
    if current_time < comm.timestamp + U256::from(3600) {
        return Err(b"REVEAL_TOO_EARLY".to_vec());
    }
    
    // Mark as revealed
    self.commitments.insert(commitment_hash, Commitment {
        committer: client,
        timestamp: comm.timestamp,
        revealed: true,
    });
    
    // Execute actual deposit
    self.deposit(sid, com_k_bytes, amount)
}
```

#### B. Add Slippage Protection
```rust
// Add to deposit.rs:
pub fn deposit_with_slippage(
    &mut self,
    sid: FixedBytes<32>,
    com_k_bytes: Vec<u8>,
    amount: U256,
    min_net_amount: U256,
) -> Result<(), Vec<u8>> {
    self.check_not_paused()?;
    
    let client = self.msg_sender();
    
    let fee = amount / U256::from(1000);
    let net_amount = amount - fee;
    
    // Check slippage
    if net_amount < min_net_amount {
        return Err(b"SLIPPAGE_TOO_HIGH".to_vec());
    }
    
    // Rest of deposit logic...
}
```

#### C. Recommend Private Mempool Usage
```rust
// DOCUMENTATION: Recommend users use private RPCs
// Flashbots Protect: https://protect.flashbots.net/
// Alchemy MEV Protection: Built-in to Alchemy RPCs
// Eden Network: Private transaction routing

// Add to README.md:
// ## MEV Protection
// 
// To protect against front-running attacks, we recommend:
// 
// 1. Use Flashbots Protect for high-value transactions
// 2. Use Alchemy RPCs (MEV protection built-in)
// 3. For sensitive operations, consider commit-reveal pattern
// 4. Set appropriate slippage tolerances
// 
// Example with Flashbots Protect:
// ```bash
// cast send --rpc-url https://rpc.flashbots.net \
//   <contract_address> \
//   "deposit(bytes32,bytes,uint256)" \
//   <sid> <com_k_bytes> <amount>
// ```
```

### Implementation Priority
- **MEDIUM:** Commit-reveal for high-value operations
- **LOW:** Slippage protection (more relevant for DEX)
- **LOW:** Private mempool recommendations (user education)

---

## 8. ZK Circuit Security Prevention

**Severity:** HIGH  
**Category:** Zero-Knowledge Circuit Security  
**Current Risk:** Audit focused on contract logic, not ZK circuit itself

### Research Sources
- Nethermind: ZK Circuit Security Guide for Engineers
- Veridise: ZK Static Analyzer (ZK Vanguard)
- Hashlock: Zero Knowledge Proof Services
- Hacken: Zero-Knowledge Proof Security

### Attack Scenario
1. ZK circuit has under-constrained signals
2. Attacker generates invalid proof that passes verification
3. Compliance checks bypassed
4. Malicious transactions accepted

### Mitigation Strategies

#### A. Conduct Professional ZK Circuit Audit
```rust
// RECOMMENDED: Hire specialized ZK audit firms
// - Nethermind Security (ZK specialists)
// - Veridise (ZK Vanguard static analysis)
// - Hashlock (ZK practice)
// - Zellic (ZK audits)
// - Trail of Bits (Formal verification)

// Audit coverage should include:
// 1. Circuit constraint analysis
// 2. Under-constrained signal detection
// 3. Public input validation
// 4. Private input leakage checks
// 5. Witness generation security
// 6. Verifier contract security
```

#### B. Use Static Analysis Tools
```rust
// RECOMMENDED: Integrate ZK Vanguard into CI/CD
// https://veridise.com/security/tools/zk-static-analyzer

// Detects:
// - Non-deterministic witness
// - Private input leakage
// - Unconstrained signals
// - Underconstrained outputs
// - Unused subcomponents
// - Witness-constraints difference

// CI/CD integration example:
// name: ZK Circuit Security Check
// on: [push, pull_request]
// jobs:
//   zk-vanguard:
//     runs-on: ubuntu-latest
//     steps:
//       - uses: actions/checkout@v2
//       - name: Run ZK Vanguard
//         run: |
//           zk-vanguard analyze ./circuits/
```

#### C. Use Audited Circuit Libraries
```rust
// RECOMMENDED: Don't implement primitives from scratch
// Use audited libraries:

// For Circom:
// - Circomlib (https://github.com/iden3/circomlib)
// - Circomlibjs (JavaScript port)

// For Noir:
// - Noir standard library
// - Aztec's Noir libraries

// For Halo2:
// - Halo2 circuits library
// - Privacy-scaling-explorations

// Example: Instead of implementing Poseidon from scratch:
use arkworks::poseidon::Poseidon; // Use audited implementation
```

#### D. Implement Circuit-Level Testing
```rust
// RECOMMENDED: Comprehensive circuit testing
// 1. Unit tests for each gadget
// 2. Integration tests for full circuit
// 3. Edge case testing (boundary values)
// 4. Adversarial testing (malicious inputs)
// 5. Performance benchmarking

// Example test structure:
#[test]
fn test_compliance_circuit_valid_proof() {
    // Test with valid inputs
    let proof = generate_valid_proof();
    assert!(verify_proof(proof));
}

#[test]
fn test_compliance_circuit_invalid_nullifier() {
    // Test with invalid nullifier
    let proof = generate_invalid_nullifier_proof();
    assert!(!verify_proof(proof));
}

#[test]
fn test_compliance_circuit_underconstrained() {
    // Test for under-constrained signals
    // Use static analysis tools
}
```

### Implementation Priority
- **HIGH:** Conduct professional ZK circuit audit before mainnet
- **HIGH:** Integrate static analysis tools in CI/CD
- **MEDIUM:** Use audited circuit libraries
- **MEDIUM:** Implement comprehensive circuit testing

---

## 9. MEV Protection Prevention

**Severity:** MEDIUM  
**Category:** MEV (Maximal Extractable Value)  
**Current Risk:** No MEV protection mechanisms

### Research Sources
- Alchemy: What is MEV and MEV Protection
- CAAW 2026: MEV Attacks in Private L2 Mempools
- ESMA: Maximal Extractable Value Implications
- Dwellir: MEV Bot Infrastructure

### Attack Scenario
1. User submits spend transaction
2. MEV bot sandwiches the transaction
3. Bot profits from price impact
4. User receives worse execution

### Mitigation Strategies

#### A. Integrate Flashbots Protect
```rust
// DOCUMENTATION: Recommend Flashbots Protect
// https://protect.flashbots.net/

// Benefits:
// - 98.5% success rate
// - 90% MEV refunds to users
// - Full gas refunds on reverted transactions
// - No charges for reverted transactions

// Integration:
// 1. Use Flashbots Protect RPC endpoint
// 2. Or use Flashbots bundle API
// 3. Document in user guide

// Example:
// ```bash
// curl -X POST https://rpc.flashbots.net/fast \
//   -H "Content-Type: application/json" \
//   -d '{
//     "jsonrpc": "2.0",
//     "method": "eth_sendPrivateTransaction",
//     "params": [{
//       "tx": "0x...",
//       "maxBlockNumber": "0x..."
//     }],
//     "id": 1
//   }'
// ```
```

#### B. Use MEV-Protected RPCs
```rust
// RECOMMENDED RPC providers with MEV protection:
// 
// 1. Alchemy (built-in MEV protection)
//    - Ethereum, Solana, Base, Arbitrum, BNB Chain
//    - No extra cost
// 
// 2. Flashbots Protect
//    - Ethereum mainnet
//    - Private mempool
// 
// 3. Eden Network
//    - Private transaction routing
//    - MEV sharing
// 
// 4. bloXroute
//    - Private mempool
//    - MEV protection

// Update README.md with recommended RPCs
```

#### C. Implement MEV-Share for Revenue
```rust
// OPTIONAL: Implement MEV-Share to capture MEV revenue
// https://docs.flashbots.net/flashbots-mev-share/overview

// Benefits:
// - Protocol captures MEV revenue
// - Users get better execution
// - Transparent MEV distribution

// Implementation:
// 1. Integrate MEV-Share API
// 2. Route transactions through MEV-Share
// 3. Distribute revenue to treasury
// 4. Report MEV revenue to users
```

### Implementation Priority
- **LOW:** Documentation and recommendations (user education)
- **LOW:** Integration with MEV-protected RPCs
- **OPTIONAL:** MEV-Share integration (revenue capture)

---

## 10. Privacy Leakage Prevention

**Severity:** MEDIUM  
**Category:** Privacy Preservation  
**Current Risk:** Nullifier pattern might be linkable with off-chain data

### Research Sources
- zk-X509: Privacy-Preserving On-Chain Identity
- Coin Bureau: Understanding Aztec Network
- Cloudflare: Anonymous Credentials
- ZK-ACE: Identity-Centric ZK Authorization

### Attack Scenario
1. Attacker has off-chain data (e.g., KYC records)
2. Attacker links nullifiers to real-world identities
3. Privacy guarantees broken
4. User activity tracked

### Mitigation Strategies

#### A. Use Signature-Based Nullifiers
```rust
// RECOMMENDED: Derive nullifiers from signatures, not public keys
// Based on zk-X509 research

// Current approach (potentially linkable):
// nullifier = H(public_key || wallet_index)

// Recommended approach (unlinkable):
// nullifier = H(Signature(sk, "nullifier" || contract_address || chain_id) || wallet_index)

// Benefits:
// - Only private key holder can generate nullifier
// - Public key doesn't reveal nullifier
// - Prevents linkage attacks

// Implementation in nimbus-core:
// Modify types.rs to include signature-based nullifier generation
pub fn generate_nullifier(
    secret_key: &Fr,
    contract_address: Address,
    chain_id: u64,
    wallet_index: u64,
) -> [u8; 32] {
    // Sign the nullifier commitment
    let message = format!("nullifier{:?}{:?}", contract_address, chain_id);
    let signature = sign(secret_key, message);
    
    // Hash signature + wallet_index
    let mut data = Vec::new();
    data.extend_from_slice(&signature);
    data.extend_from_slice(&wallet_index.to_be_bytes::<8>());
    
    let mut nullifier = [0u8; 32];
    // Use proper hash function
    nullifier.copy_from_slice(&hash(&data)[..32]);
    
    nullifier
}
```

#### B. Implement Domain Separation
```rust
// RECOMMENDED: Use domain separation for cross-chain privacy
// Based on ZK-ACE research

// Add to verification.rs:
pub fn verify_compliance(
    &self,
    root: FixedBytes<32>,
    nullifier: FixedBytes<32>,
    recipient: Address,
    amount: U256,
    proof_a_neg_bytes: Vec<u8>,
    proof_b_bytes: Vec<u8>,
    proof_c_bytes: Vec<u8>,
    domain_separator: FixedBytes<32>, // NEW: Domain separator
) -> Result<bool, Vec<u8>> {
    // 1. Verify clean root is registered
    if !self.clean_association_roots.get(root) {
        return Ok(false);
    }
    
    // 2. Load the compliance Verifying Key (VK)
    let (vk_alpha, vk_beta, vk_gamma, vk_delta, vk_ic) = self.get_compliance_vk();
    
    // 3. Compute the public inputs G1 linear combination with domain separation
    let public_inputs_g1_bytes = self.compute_public_inputs_g1_with_domain(
        &vk_ic,
        root,
        nullifier,
        recipient,
        amount,
        domain_separator, // NEW: Include domain separator
    )?.to_vec();
    
    // 4. Verify ZK Proof
    let is_zk_valid = self.verify_groth16_proof(
        proof_a_neg_bytes,
        proof_b_bytes,
        proof_c_bytes,
        public_inputs_g1_bytes,
        vk_alpha.to_vec(),
        vk_beta.to_vec(),
        vk_gamma.to_vec(),
        vk_delta.to_vec(),
    )?;
    
    Ok(is_zk_valid)
}

// Add new function:
pub(crate) fn compute_public_inputs_g1_with_domain(
    &self,
    vk_ic: &[[u8; 128]; 5],
    root: FixedBytes<32>,
    nullifier: FixedBytes<32>,
    recipient: Address,
    amount: U256,
    domain_separator: FixedBytes<32>,
) -> Result<[u8; 128], Vec<u8>> {
    // Include domain_separator in public inputs
    // This prevents cross-domain proof replay
    // ... implementation ...
}
```

#### C. Implement Ephemeral Agent Identifiers
```rust
// RECOMMENDED: Support ephemeral agent identities
// Based on roadmap Section 10

// Benefits:
// - Each agent session has unique identity
// - Prevents address clustering
// - Enhances privacy for AI agents

// Implementation:
// 1. Generate ephemeral key pair per session
// 2. Derive nullifier from ephemeral key
// 3. Link to master identity via ZK proof
// 4. Master identity remains hidden

// Add to types.rs:
pub struct EphemeralIdentity {
    pub ephemeral_public_key: G1Affine,
    pub master_identity_commitment: [u8; 32],
    pub session_id: FixedBytes<32>,
}
```

### Implementation Priority
- **MEDIUM:** Implement signature-based nullifiers
- **MEDIUM:** Add domain separation for cross-chain
- **LOW:** Ephemeral agent identifiers (roadmap item)

---

## Implementation Roadmap

### Phase 1: Critical (Before Mainnet)
1. **Oracle Manipulation Prevention** - Integrate Chainlink Price Feeds
2. **Compliance Root Centralization** - Implement timelock + multi-sig
3. **RWA Token Risks** - Implement KYC allowlist
4. **Cross-Chain Finality** - Add replay protection

### Phase 2: High Priority (Post-Mainnet)
5. **ZK Circuit Security** - Conduct professional audit
6. **Gas Griefing** - Fix state consistency + add error handling
7. **Privacy Leakage** - Implement signature-based nullifiers

### Phase 3: Medium Priority (Ongoing)
8. **Timestamp Manipulation** - Consider block numbers
9. **Front-Running** - Implement commit-reveal for high-value ops
10. **MEV Protection** - Documentation + RPC recommendations

---

## Additional Resources

### Security Auditors
- **Nethermind Security** - ZK specialists, formal verification
- **Trail of Bits** - Smart contract audits, formal verification
- **OpenZeppelin** - Smart contract security, Stylus audits
- **Veridise** - ZK static analysis (ZK Vanguard)
- **Zellic** - Smart contract and ZK audits

### Tools & Infrastructure
- **Chainlink** - Price feeds, CCIP, compliance
- **Flashbots** - MEV protection
- **Safe (Gnosis Safe)** - Multi-sig wallets
- **Circomlib / Noir stdlib** - Audited ZK libraries
- **ZK Vanguard** - ZK static analysis

### Standards & Best Practices
- **ERC-3643 (T-REX)** - RWA compliance standard
- **OWASP Smart Contract Top 10 2026** - Security standards
- **SEAL Frameworks** - Secure multisig best practices
- **Chainlink CCIP** - Cross-chain security

---

**Disclaimer:** These mitigation strategies are based on 2025/2026 security research and best practices. Implementation should be validated through professional security audits before mainnet deployment.
