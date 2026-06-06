# Node Infrastructure Security Audit: Nimbus Relayer/Keeper Bot

**Audit Date:** June 5, 2026  
**Auditor:** Cascade (Senior DevOps & Cloud Infrastructure Security Auditor)  
**Scope:** Infrastructure & Off-Chain Logic Security - Backend/Relayer/Keeper Bot  
**Focus Areas:** Secrets management, race conditions, DoS prevention, economic exploitation

---

## Research Sources & References

### 2026 Attack Trends Against MEV Bots, Relayers, Keeper Nodes

**Sources:**
1. **Shutter Network: How to Protect Yourself Against Malicious MEV in 2026** - https://blog.shutter.network/how-to-protect-yourself-against-malicious-mev-in-2026
   - Encrypted mempools as long-term solution
   - Private mempools as interim protection
   - MEV bots exploit public mempool visibility

2. **CryptoSlate: Ethereum bots burning 50%+ of gas fees** - https://cryptoslate.com/ethereum-bots-are-burning-over-50-of-gas-fees-so-eth-now-needs-privacy-just-to-scale
   - MEV extraction at economic crisis levels
   - Privacy becoming essential for scaling

3. **arXiv: Autonomous Agents on Blockchains (2026)** - https://arxiv.org/html/2601.04583v1
   - Adversarial execution environments
   - MEV formalization and protection strategies
   - Trust assumptions and dependencies

4. **JumpServer: Secrets Management Best Practices 2026** - https://www.jumpserver.com/blog/secret-management-best-practices-2026
   - Centralize secrets in dedicated vaults
   - Eliminate hardcoded secrets
   - Implement least privilege access

5. **Cycode: Best Secrets Management Tools 2026** - https://cycode.com/blog/best-secrets-management-tools
   - Coverage across code, pipelines, infrastructure
   - Detection vs control strategies
   - CI/CD integration requirements

6. **Hacken: Blockchain Security Vulnerabilities** - https://hacken.io/insights/blockchain-security-vulnerabilities
   - Replay attack prevention with nonces
   - Length extension attacks
   - Double-spend mitigation

7. **Smart Contract Hacking: Frontrunning Attacks (2026)** - https://smartcontractshacking.com/attacks/frontrunning-attacks
   - ERC-20 approve race condition
   - MEV supply chain analysis
   - Front-running prevention strategies

8. **Beltsys Labs: What Is RPC in Blockchain 2026** - https://beltsys.com/en/blog/what-is-rpc-in-blockchain
   - Multi-provider fallback requirements
   - Rate limiting for RPC endpoints
   - RPC centralization risks

9. **Bitcoin Foundation: KelpDAO $292M Hack Analysis** - https://bitcoinfoundation.org/news/crimes-and-fraud-news/how-kelpdao-lost-292m-inside-2026s-biggest-defi-hack-and-what-went-wrong
   - RPC poisoning attack vector
   - Single point of failure risks
   - DVN failure implications

10. **Alchemy: Enterprise RPC Infrastructure Evaluation** - https://www.alchemy.com/blog/blockchain-rpc-infrastructure-evaluation-guide-for-enterprises
    - DDoS protection requirements
    - Audit logs and SIEM integration
    - Rate limiting strategies

11. **Cube Exchange: Oracle Manipulation** - https://www.cube.exchange/what-is/oracle-manipulation
    - Relayer/keeper operational dependencies
    - Oracle security assumptions
    - Asynchronous off-chain aggregation risks

12. **Quillaudits: Perp DEX Architecture & Security** - https://www.quillaudits.com/blog/dex/perp-dex-architecture-and-security
    - Keeper bot delay exploitation
    - Liquidation system dependencies
    - MEV and network-level exploits

13. **Hacken: Flash Loan Attacks** - https://hacken.io/discover/flash-loan-attacks
    - KiloEx $7.5M exploit (access control via trusted forwarder)
    - Keeper/relayer as attack surface
    - Oracle threat model considerations

14. **Frontier Enterprise: 2026 Cybersecurity Predictions** - https://www.frontier-enterprise.com/the-2026-cybersecurity-predictions-bonanza
    - AI agents as internal threat vectors
    - Software supply chain attacks
    - Botnet and credential rental services

---

# Critical Findings

## 1. CRITICAL: Insecure Default Key Fallback

**Severity:** CRITICAL  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/kms.rs:79-81`  
**Category:** Secrets Management / Private Key Handling  
**Source:** JumpServer Secrets Management Best Practices 2026

**Vulnerable Code:**
```rust
// kms.rs:79-81
println!("WARNING: No share key found via Vault or environment variables. Using default insecure key.");
(nimbus_core::Fr::from(12345u64), env_index)
```

**Issue:** The system falls back to a hardcoded insecure default key (`Fr::from(12345u64)`) when no Vault or environment variable is configured. This is a catastrophic security failure that allows anyone who knows this default to sign transactions on behalf of the protocol.

**Attack Scenario:**
1. Attacker discovers the default key value (publicly visible in source code)
2. Attacker crafts malicious threshold signature requests
3. System signs with default key
4. Attacker can drain funds or bypass compliance checks

**Impact:** Complete protocol compromise, unauthorized transaction signing, fund drainage

**Reference:** JumpServer best practices explicitly warn against hardcoded secrets and recommend centralized vault storage with no fallback to insecure defaults.

---

## 2. CRITICAL: In-Memory Secret Storage Without Encryption

**Severity:** CRITICAL  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/state.rs:16-17`  
**Category:** Secrets Management / Memory Security  
**Source:** Cycode Secrets Management Tools 2026

**Vulnerable Code:**
```rust
// state.rs:16-17
pub share_sk: Arc<nimbus_core::Fr>,
pub share_index: u32,
```

**Issue:** The BLS share key is stored in plain text in memory (`Arc<Fr>`) without encryption. This exposes the key to:
- Memory dumps from compromised servers
- Core dump analysis
- Debugging tools
- Process inspection by malicious insiders or attackers

**Attack Scenario:**
1. Attacker gains server access (via vulnerability, insider threat, or supply chain attack)
2. Attacker dumps process memory
3. Attacker extracts `share_sk` from memory dump
4. Attacker can sign transactions indefinitely

**Impact:** Private key compromise, unauthorized signing, protocol takeover

**Reference:** Cycode recommends runtime monitoring for secrets in memory and encryption of sensitive data at rest and in transit.

---

## 3. CRITICAL: No Rate Limiting on API Endpoints

**Severity:** CRITICAL  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/main.rs:30-38`  
**Category:** Denial of Service / Rate Limiting  
**Source:** Alchemy RPC Infrastructure Evaluation Guide

**Vulnerable Code:**
```rust
// main.rs:30-38
let app = Router::new()
    .route("/health", get(health_check))
    .route("/api/deposit", post(handle_deposit))
    .route("/api/reveal", post(handle_reveal))
    .route("/api/spend", post(handle_spend))
    .route("/api/x402/verify", post(handle_x402_verify))
    .route("/api/sign-share", post(handle_sign_share))
    .route("/api/leader/sign", post(handle_leader_sign))
    .with_state(state);
```

**Issue:** All API endpoints have NO rate limiting. An attacker can:
- Flood the spend queue with malicious requests
- Exhaust memory with in-memory session storage
- Cause denial of service by overwhelming the server
- Manipulate batch processing economics

**Attack Scenario:**
1. Attacker sends 10,000 spend requests per second
2. In-memory queue grows unbounded
3. Server runs out of memory
4. Legitimate users cannot submit transactions
5. Relayer wallet balance depleted by gas costs

**Impact:** Service disruption, resource exhaustion, economic loss

**Reference:** Alchemy explicitly recommends rate limiting for RPC endpoints and DDoS protection for blockchain infrastructure.

---

## 4. HIGH: In-Memory Nullifier Storage (Lost on Restart)

**Severity:** HIGH  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/state.rs:13-15`  
**Category:** State Persistence / Replay Attack Prevention  
**Source:** Hacken Blockchain Security Vulnerabilities

**Vulnerable Code:**
```rust
// state.rs:13-15
pub sessions: Arc<Mutex<Vec<Session>>>,
pub spend_queue: Arc<Mutex<Vec<crate::dto::SpendRequest>>>,
pub nullifiers: Arc<Mutex<Vec<String>>>,
```

**Issue:** Nullifiers, sessions, and spend queue are stored in-memory and lost on restart. This creates:
- Replay attack vulnerability after restart
- Loss of pending transactions
- Inconsistent state between restarts
- No audit trail

**Attack Scenario:**
1. User submits spend transaction
2. Transaction queued but not yet processed
3. Server restarts (crash, deployment, maintenance)
4. Nullifier list cleared from memory
5. Attacker resubmits same transaction (replay attack)
6. Double-spend succeeds

**Impact:** Double-spending, fund loss, state inconsistency

**Reference:** Hacken recommends effective use of nonces for replay attack prevention and persistent storage for critical state.

---

## 5. HIGH: No Blockchain Finality Checks

**Severity:** HIGH  
**Location:** `nimbus-node/src/handlers/spend.rs:43-149`  
**Category:** Re-org Vulnerability / Finality  
**Source:** Gate Wiki: Monitoring Blockchain Reorganization Events

**Vulnerable Code:**
```rust
// spend.rs:109-143
// Simulate transaction submission on-chain
for request in queue.iter() {
    // Register nullifiers to prevent double spend
    nulls.push(request.nullifier.clone());
    // ... process transaction ...
}
```

**Issue:** The batch processing logic has no finality checks. On Layer 2 networks (Arbitrum, Optimism, Base), blocks can be reorganized for up to 1 hour. The system:
- Doesn't wait for finality before marking nullifiers as spent
- Doesn't handle re-orgs
- Doesn't validate block confirmations
- Assumes immediate finality

**Attack Scenario:**
1. Batch transaction submitted to L2
2. Nullifiers marked as spent in memory
3. L2 reorg occurs (common during network congestion)
4. Transactions reverted on-chain
5. Nullifiers still marked as spent in memory
6. Users cannot retry transactions
7. Funds locked

**Impact:** User funds locked, inability to retry, state inconsistency

**Reference:** Gate Wiki documents reorg events on Polygon and the need for finality monitoring on L2 networks.

---

## 6. HIGH: No RPC Provider Fallback or Retry Logic

**Severity:** HIGH  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/http.rs:5-39, 41-86`  
**Category:** RPC Failure Handling / Resilience  
**Source:** Beltsys Labs: What Is RPC in Blockchain 2026

**Vulnerable Code:**
```rust
// http.rs:5-39
pub async fn post_http(url: &str, body: &str) -> Result<String, String> {
    let clean_url = url.trim_start_matches("http://").trim_start_matches("https://");
    let parts: Vec<&str> = clean_url.splitn(2, '/').collect();
    let host_port = parts[0];
    let path = if parts.len() > 1 { format!("/{}", parts[1]) } else { "/".to_string() };
    
    let mut stream = tokio::net::TcpStream::connect(host_port)
        .await
        .map_err(|e| format!("Connect failed: {}", e))?;
    // ... no timeout, no retry, no fallback ...
}
```

**Issue:** The HTTP client has:
- No timeout configuration (can hang indefinitely)
- No retry logic for transient failures
- No RPC provider fallback (single point of failure)
- No circuit breaker pattern
- No exponential backoff

**Attack Scenario:**
1. Primary RPC provider goes down (Alchemy outage, Infura downtime)
2. HTTP connection hangs indefinitely (no timeout)
3. All guardian signature calls fail
4. Threshold signing operation fails
5. Batch processing stalls
6. Users cannot complete transactions

**Impact:** Service disruption, single point of failure, poor user experience

**Reference:** Beltsys Labs recommends multi-provider fallback, timeout configuration, and monitoring for RPC infrastructure.

---

## 7. HIGH: No Request Validation or Size Limits

**Severity:** HIGH  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/handlers/*.rs`  
**Category:** Input Validation / DoS Prevention  
**Source:** Red Button: Why Rate Limits Fail in Distributed DDoS Attacks

**Vulnerable Code:**
```rust
// handlers/deposit.rs:6-27
pub async fn handle_deposit(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<DepositRequest>,
) -> Json<DepositResponse> {
    let mut sessions = state.sessions.lock().await;
    
    // Store session - NO VALIDATION
    sessions.push(Session {
        session_id: payload.session_id.clone(),
        com_k: payload.com_k,
        amount: payload.amount,
        resolved: false,
        masking_key: None,
    });
```

**Issue:** API handlers have:
- No request size limits
- No input validation (session_id length, amount bounds)
- No rate limiting per IP/user
- No request throttling
- No memory usage monitoring

**Attack Scenario:**
1. Attacker sends deposit request with 1MB session_id
2. In-memory vector grows unbounded
3. Server memory exhausted
4. Service crashes
5. All pending transactions lost

**Impact:** Memory exhaustion, service crash, data loss

**Reference:** Red Button explains how rate limits fail under distributed DDoS attacks and the need for comprehensive request validation.

---

## 8. MEDIUM: Hardcoded Gas Prices (Economic Manipulation Risk)

**Severity:** MEDIUM  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/handlers/spend.rs:54-59`  
**Category:** Economic Exploitation / Oracle Manipulation  
**Source:** Cube Exchange: Oracle Manipulation

**Vulnerable Code:**
```rust
// spend.rs:54-59
// 1. L2 Gas Parameters & Economics (arXiv:2505.19556 - Batch-Calibrated)
let eth_usd_price = 3500.0;
let l2_gas_price_gwei = 0.1;
let l2_gas_price_eth = l2_gas_price_gwei * 1e-9;
let l2_exec_gas_per_tx = 120_000.0;
let l1_calldata_gas_per_tx = 80_000.0;
let l1_base_batch_fee_eth = 0.005;
```

**Issue:** Gas prices are hardcoded:
- No dynamic price fetching from oracles
- No validation against on-chain gas prices
- Susceptible to gas price manipulation
- Can cause undercharging or overcharging users

**Attack Scenario:**
1. Actual gas price spikes to 1000 gwei (network congestion)
2. System still charges based on 0.1 gwei
3. Relayer pays 10,000x actual gas cost
4. Relayer wallet depleted
5. Service stops processing transactions

**Impact:** Economic loss, service disruption, insolvency risk

**Reference:** Cube Exchange discusses oracle manipulation risks and the need for dynamic price validation.

---

## 9. MEDIUM: No Slippage Protection in Batch Processing

**Severity:** MEDIUM  
**Location:** `nimbus-node/src/handlers/spend.rs:114-120`  
**Category:** Economic Exploitation / MEV  
**Source:** Smart Contract Hacking: Frontrunning Attacks

**Vulnerable Code:**
```rust
// spend.rs:114-120
// Deduct gas cost + markup from user's spend amount
let charge_usdc_units = (total_charge_per_tx_usd * 1_000_000.0) as u64;
let net_payout = if request.amount >= charge_usdc_units {
    request.amount - charge_usdc_units
} else {
    0
};
```

**Issue:** No slippage protection:
- Users cannot specify minimum acceptable payout
- Batch processing can be front-run
- No deadline enforcement
- No price impact protection

**Attack Scenario:**
1. User submits spend for 1000 USDC
2. MEV bot front-runs and causes price impact
3. Gas costs spike during processing
4. User receives 0 USDC (amount < charge)
5. User loses entire transaction value

**Impact:** User fund loss, poor user experience, MEV extraction

**Reference:** Smart Contract Hacking course discusses front-running prevention including slippage protection and deadline enforcement.

---

## 10. MEDIUM: No Minimum Balance Checks for Relayer Wallet

**Severity:** MEDIUM  
**Location:** `nimbus-node/src/handlers/spend.rs:82-97`  
**Category:** Economic Exploitation / Insolvency Risk  
**Source:** Quillaudits: Perp DEX Architecture & Security

**Vulnerable Code:**
```rust
// spend.rs:82-97
// Update relayer wallet balance and profit
{
    let mut relayer_bal = state.relayer_wallet_balance_eth.lock().await;
    *relayer_bal -= total_batch_cost_eth;
    let mut relayer_profit = state.relayer_accumulated_profit_usdc.lock().await;
    *relayer_profit += total_batch_profit_usd;
    // ... no minimum balance check ...
}
```

**Issue:** No minimum balance checks:
- Relayer can go negative (in simulation)
- No insolvency protection
- No automatic refill mechanism
- No alerting for low balance

**Attack Scenario:**
1. Attacker floods queue with high-value transactions
2. Gas costs exceed relayer wallet balance
3. Balance goes negative
4. Transactions fail on-chain
5. Users lose funds (already deducted from amounts)
6. Relayer becomes insolvent

**Impact:** Insolvency risk, user fund loss, protocol failure

**Reference:** Quillaudits discusses keeper bot delay exploitation and the need for economic safeguards in automated systems.

---

## 11. MEDIUM: No Transaction Deduplication Across Restarts

**Severity:** MEDIUM  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/handlers/spend.rs:10-20`  
**Category:** State Consistency / Replay Attack  
**Source:** Hacken: Flash Loan Attacks

**Vulnerable Code:**
```rust
// spend.rs:10-20
pub async fn handle_spend(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SpendRequest>,
) -> Json<SpendResponse> {
    let mut queue = state.spend_queue.lock().await;
    
    // Check if nullifier has already been processed (on-chain check)
    let nulls = state.nullifiers.lock().await;
    if nulls.contains(&payload.nullifier) {
        return Json(SpendResponse {
            status: "REJECTED".to_string(),
            message: "Double-spending detected. Nullifier already exists.".to_string(),
            queue_position: 0,
        });
    }
```

**Issue:** Nullifier check is in-memory only:
- Lost on restart
- No persistent database
- No on-chain verification
- No transaction ID tracking

**Attack Scenario:**
1. User submits spend transaction
2. Transaction queued
3. Server restarts
4. Nullifier check passes (list cleared)
5. Attacker resubmits same transaction
6. Double-spend succeeds

**Impact:** Double-spending, fund loss, state inconsistency

**Reference:** Hacken's KiloEx analysis shows how access control failures in trusted forwarders/keepers can lead to exploits.

---

## 12. MEDIUM: No Key Rotation Mechanism

**Severity:** MEDIUM  
**Location:** `nimbus-node/src/kms.rs:6-81`  
**Category:** Secrets Management / Key Lifecycle  
**Source:** Cycode: Secrets Management Best Practices

**Vulnerable Code:**
```rust
// kms.rs:6-81
pub async fn load_share_key() -> (nimbus_core::Fr, u32) {
    // ... load from Vault or env var ...
    // No rotation logic
    // No expiration
    // No versioning
}
```

**Issue:** No key rotation:
- Keys loaded once at startup
- No rotation mechanism
- No key versioning
- No automatic expiration

**Attack Scenario:**
1. Private key compromised (insider threat, breach)
2. No way to rotate without full restart
3. Attacker continues to sign transactions
4. Protocol remains compromised until manual intervention

**Impact:** Prolonged compromise, inability to respond to incidents

**Reference:** Cycode recommends automated key rotation and expiration as part of secrets management best practices.

---

## 13. LOW: No Audit Logging for Critical Operations

**Severity:** LOW  
**Location:** All handlers  
**Category:** Observability / Incident Response  
**Source:** Alchemy: Enterprise RPC Infrastructure Evaluation

**Issue:** No structured audit logging:
- Only `println!` statements (not production-ready)
- No SIEM integration
- No log retention policy
- No security event correlation

**Impact:** Difficult incident response, poor security posture, compliance issues

**Reference:** Alchemy recommends detailed audit logs with SIEM integration for enterprise security.

---

## 14. LOW: No Health Check for External Dependencies

**Severity:** LOW  
**Status:** **REMEDIATED (6 Juni 2026)**
**Location:** `nimbus-node/src/handlers/health.rs:6-20`  
**Category:** Observability / Health Monitoring

**Vulnerable Code:**
```rust
// health.rs:6-20
pub async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<HealthResponse> {
    let queue = state.spend_queue.lock().await;
    let nulls = state.nullifiers.lock().await;
    let relayer_wallet_balance_eth = *state.relayer_wallet_balance_eth.lock().await;
    let relayer_accumulated_profit_usdc = *state.relayer_accumulated_profit_usdc.lock().await;
    Json(HealthResponse {
        status: "OK".to_string(),
        queued_transactions: queue.len(),
        processed_nullifiers: nulls.len(),
        relayer_wallet_balance_eth,
        relayer_accumulated_profit_usdc,
    })
}
```

**Issue:** Health check doesn't verify:
- RPC provider connectivity
- Vault/OpenBao availability
- Guardian node health
- Database connectivity (when implemented)

**Impact:** Poor observability, delayed incident detection

---

# Additional Findings Beyond User Questions

## 15. MEDIUM: No Circuit Breaker Pattern

**Severity:** MEDIUM  
**Category:** Resilience / Fault Tolerance

**Issue:** No circuit breaker pattern for:
- RPC calls to guardians
- Vault/OpenBao access
- Blockchain RPC providers

**Impact:** Cascading failures, poor resilience

**Mitigation:** Implement circuit breaker library (e.g., `tokio-circuit-breaker`)

---

## 16. MEDIUM: No Request IDempotency

**Severity:** MEDIUM  
**Category:** API Design / Idempotency

**Issue:** No idempotency keys:
- Duplicate requests can cause duplicate processing
- No request deduplication
- No client-side retry safety

**Impact:** Duplicate transactions, user confusion

**Mitigation:** Implement idempotency keys for all state-changing operations

---

## 17. LOW: No Metrics/Telemetry

**Severity:** LOW  
**Category:** Observability / Monitoring

**Issue:** No metrics collection:
- No Prometheus metrics
- No distributed tracing
- No performance monitoring

**Impact:** Poor operational visibility, difficult troubleshooting

**Mitigation:** Integrate Prometheus + OpenTelemetry

---

## 18. LOW: No Graceful Shutdown

**Severity:** LOW  
**Category:** Operational Safety

**Issue:** No graceful shutdown:
- In-flight transactions lost on restart
- No drain period for queue
- No signal handling

**Impact:** Data loss, poor user experience

**Mitigation:** Implement graceful shutdown with queue drain

---

# Implementation Roadmap

### Phase 1: Critical (Before Production)
1. **Remove insecure default key fallback** - Fail fast if no key configured
2. **Implement persistent database** - Replace in-memory storage with PostgreSQL/SQLite
3. **Add rate limiting** - Implement per-IP and per-user rate limits
4. **Add request validation** - Validate all inputs and set size limits

### Phase 2: High Priority (Post-Production)
5. **Implement RPC provider fallback** - Multi-provider with retry logic
6. **Add finality checks** - Wait for L2 finality before marking nullifiers spent
7. **Implement memory encryption** - Encrypt secrets in memory (mlock, secure memory)
8. **Add minimum balance checks** - Prevent relayer insolvency

### Phase 3: Medium Priority (Ongoing)
9. **Implement dynamic gas pricing** - Fetch from Chainlink or on-chain
10. **Add slippage protection** - Allow users to specify minimum payout
11. **Implement key rotation** - Automated key rotation mechanism
12. **Add audit logging** - Structured logs with SIEM integration

### Phase 4: Low Priority (Operational Excellence)
13. **Add circuit breakers** - Improve resilience
14. **Implement idempotency** - Request deduplication
15. **Add metrics/telemetry** - Prometheus + OpenTelemetry
16. **Implement graceful shutdown** - Queue drain on shutdown

---

# Recommended Mitigation Code

## 1. Remove Insecure Default Key

```rust
// kms.rs - Replace line 79-81:
// OLD:
// println!("WARNING: No share key found via Vault or environment variables. Using default insecure key.");
// (nimbus_core::Fr::from(12345u64), env_index)

// NEW:
println!("ERROR: No share key found via Vault or environment variables.");
println!("CRITICAL: Cannot start without valid BLS share key.");
println!("Please configure NIMBUS_VAULT_TOKEN or NIMBUS_SHARE_KEY environment variable.");
std::process::exit(1);
```

## 2. Implement Rate Limiting

```rust
// Add to Cargo.toml:
// tower-governor = "0.4"

// Add to main.rs:
use tower_governor::{Governor, GovernorConfigBuilder};
use tower_governor::key_extractor::SmartIpKeyExtractor;

// Configure rate limiter
let governor_conf = Box::new(
    GovernorConfigBuilder::default()
        .per_second(10) // 10 requests per second
        .burst_size(30) // Allow burst of 30
        .finish()
        .unwrap(),
);

let app = Router::new()
    .route("/health", get(health_check))
    .route("/api/deposit", post(handle_deposit))
    .route("/api/reveal", post(handle_reveal))
    .route("/api/spend", post(handle_spend))
    .route("/api/x402/verify", post(handle_x402_verify))
    .route("/api/sign-share", post(handle_sign_share))
    .route("/api/leader/sign", post(handle_leader_sign))
    .layer(Governor {
        config: governor_conf,
        key_extractor: SmartIpKeyExtractor,
    })
    .with_state(state);
```

## 3. Implement Persistent Database

```rust
// Add to Cargo.toml:
// sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "sqlite"] }

// Replace state.rs:
use sqlx::{Pool, Postgres};

pub struct AppState {
    pub db_pool: Pool<Postgres>,
    pub share_sk: Arc<nimbus_core::Fr>,
    pub share_index: u32,
}

impl AppState {
    pub async fn new(share_sk: nimbus_core::Fr, share_index: u32) -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");
        
        let db_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
            .expect("Failed to connect to database");
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&db_pool).await
            .expect("Failed to run migrations");
        
        Self {
            db_pool,
            share_sk: Arc::new(share_sk),
            share_index,
        }
    }
}

// Update handlers to use database instead of in-memory storage
pub async fn handle_spend(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SpendRequest>,
) -> Json<SpendResponse> {
    // Check nullifier in database
    let nullifier_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM nullifiers WHERE nullifier = $1)"
    )
    .bind(&payload.nullifier)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(false);
    
    if nullifier_exists {
        return Json(SpendResponse {
            status: "REJECTED".to_string(),
            message: "Double-spending detected. Nullifier already exists.".to_string(),
            queue_position: 0,
        });
    }
    
    // Insert into spend_queue table
    sqlx::query(
        "INSERT INTO spend_queue (nullifier, sig_hex, recipient, amount, eip7702_auth, cross_chain) 
         VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(&payload.nullifier)
    .bind(&payload.sig_hex)
    .bind(&payload.recipient)
    .bind(payload.amount as i64)
    .bind(serde_json::to_string(&payload.eip7702_auth).unwrap_or_default())
    .bind(serde_json::to_string(&payload.cross_chain).unwrap_or_default())
    .execute(&state.db_pool)
    .await
    .unwrap();
    
    // Get queue position
    let position = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM spend_queue"
    )
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0) as usize;
    
    Json(SpendResponse {
        status: "QUEUED".to_string(),
        message: "Spend transaction accepted into batching queue".to_string(),
        queue_position: position,
    })
}
```

## 4. Implement RPC Provider Fallback

```rust
// Add to Cargo.toml:
// reqwest = { version = "0.11", features = ["json"] }

// Create rpc.rs:
use reqwest::Client;
use std::time::Duration;

pub struct RpcClient {
    clients: Vec<Client>,
    urls: Vec<String>,
    timeout: Duration,
}

impl RpcClient {
    pub fn new(urls: Vec<String>) -> Self {
        let clients = urls.iter()
            .map(|_| Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap())
            .collect();
        
        Self {
            clients,
            urls,
            timeout: Duration::from_secs(10),
        }
    }
    
    pub async fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        let mut last_error = None;
        
        for (i, (client, url)) in self.clients.iter().zip(&self.urls).enumerate() {
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
                "id": 1
            });
            
            match client.post(url)
                .json(&body)
                .send()
                .await
            {
                Ok(response) => {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => return Ok(json),
                        Err(e) => last_error = Some(format!("Parse error from provider {}: {}", i, e)),
                    }
                }
                Err(e) => {
                    last_error = Some(format!("RPC provider {} failed: {}", i, e));
                    continue;
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| "All RPC providers failed".to_string()))
    }
}

// Use in threshold.rs:
let rpc_client = RpcClient::new(vec![
    std::env::var("RPC_URL_1").unwrap_or_else(|_| "https://arb1.arbitrum.io/rpc".to_string()),
    std::env::var("RPC_URL_2").unwrap_or_else(|_| "https://arb1.arbitrum.io/rpc".to_string()),
]);
```

## 5. Add Finality Checks

```rust
// Add to spend.rs:
pub async fn process_spend_batch(state: &AppState) {
    let mut queue = state.spend_queue.lock().await;
    if queue.is_empty() {
        return;
    }
    
    // Wait for L2 finality (Arbitrum: ~1 hour = 300 blocks)
    let current_block = rpc_client.call("eth_blockNumber", serde_json::json!([])).await?;
    let current_block_num: u64 = serde_json::from_value(current_block["result"].clone())?;
    let finality_blocks = 300;
    
    for request in queue.iter() {
        // Check if transaction is finalized
        let tx_receipt = rpc_client.call("eth_getTransactionReceipt", 
            serde_json::json!([request.tx_hash])).await?;
        
        if let Some(receipt) = tx_receipt.get("result") {
            let block_num: u64 = serde_json::from_value(receipt["blockNumber"].clone())?;
            
            if current_block_num - block_num < finality_blocks {
                println!("Transaction {} not yet finalized, skipping", request.tx_hash);
                continue;
            }
        }
        
        // Process transaction
        nulls.push(request.nullifier.clone());
        // ... rest of processing ...
    }
}
```

## 6. Add Request Validation

```rust
// Add validation middleware to main.rs:
use axum::extract::Request;
use axum::middleware::Next;

async fn request_validation_middleware(
    req: Request,
    next: Next,
) -> Result<Request, String> {
    // Check request size
    let content_length = req.headers()
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    
    const MAX_REQUEST_SIZE: usize = 1024 * 1024; // 1MB
    if content_length > MAX_REQUEST_SIZE {
        return Err("Request too large".to_string());
    }
    
    Ok(req)
}

// Apply to router:
let app = Router::new()
    .route("/api/deposit", post(handle_deposit))
    .route("/api/reveal", post(handle_reveal))
    .route("/api/spend", post(handle_spend))
    .route("/api/x402/verify", post(handle_x402_verify))
    .route("/api/sign-share", post(handle_sign_share))
    .route("/api/leader/sign", post(handle_leader_sign))
    .layer(axum::middleware::from_fn(request_validation_middleware))
    .with_state(state);
```

## 7. Add Minimum Balance Checks

```rust
// Add to spend.rs:
const MIN_RELAYER_BALANCE_ETH: f64 = 1.0; // 1 ETH minimum

pub async fn process_spend_batch(state: &AppState) {
    let mut queue = state.spend_queue.lock().await;
    if queue.is_empty() {
        return;
    }
    
    // Check relayer balance before processing
    let relayer_bal = *state.relayer_wallet_balance_eth.lock().await;
    if relayer_bal < MIN_RELAYER_BALANCE_ETH {
        println!("ERROR: Relayer balance below minimum ({:.5} ETH < {:.5} ETH)", 
            relayer_bal, MIN_RELAYER_BALANCE_ETH);
        println!("CRITICAL: Refusing to process batch. Please refill relayer wallet.");
        // Send alert to monitoring system
        send_alert("Relayer balance below minimum threshold").await;
        return;
    }
    
    // Calculate total batch cost
    let total_batch_cost_eth = batch_gas_cost_per_tx_eth * batch_size as f64;
    
    // Check if balance is sufficient
    if relayer_bal < total_batch_cost_eth + MIN_RELAYER_BALANCE_ETH {
        println!("ERROR: Insufficient balance for batch ({:.5} ETH needed, {:.5} ETH available)",
            total_batch_cost_eth, relayer_bal);
        return;
    }
    
    // Process batch
    // ... rest of processing ...
}
```

---

# Additional Resources

### Secrets Management
- **HashiCorp Vault** - Enterprise secrets engine
- **OpenBao** - Open-source Vault alternative
- **AWS Secrets Manager** - Cloud-native secrets
- **Doppler** - Developer-friendly secrets platform
- **1Password Developer** - Password manager for devs

### RPC Infrastructure
- **Alchemy** - Managed RPC with MEV protection
- **Infura** - Ethereum infrastructure
- **QuickNode** - Multi-chain RPC
- **Chainstack** - Global node infrastructure
- **Ankr** - Decentralized RPC network

### Monitoring & Observability
- **Prometheus** - Metrics collection
- **Grafana** - Visualization
- **OpenTelemetry** - Distributed tracing
- **Sentry** - Error tracking
- **Datadog** - APM and monitoring

### Rate Limiting
- **tower-governor** - Rust rate limiting
- **Redis** - Distributed rate limiting
- **Cloudflare** - Edge rate limiting
- **AWS API Gateway** - Managed rate limiting

---

**Disclaimer:** This audit is based on code review and 2025/2026 security research. A full production deployment should include penetration testing, load testing, and 24/7 monitoring. All mitigation code should be tested thoroughly before deployment.
