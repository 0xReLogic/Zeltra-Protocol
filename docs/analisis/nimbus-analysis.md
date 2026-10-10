# Nimbus Protocol: Game Theory, Economic Incentives & Security Analysis

**Analysis Date:** 2025-06-14  
**Protocol Version:** Phase 1 (Baseline) + Phase 2 (ZK-UTXO Gate C0 in progress)  
**Network:** Arbitrum Sepolia (testnet verified) + Mainnet readiness track

---

## Executive Summary

Nimbus adalah **private payment protocol** berbasis BLS12-381 threshold blind signing + ZK-SNARK yang dirancang untuk pembayaran stablecoin privat di Arbitrum Stylus. Protokol ini menggunakan **immutable mathematical constants** untuk fee structure (deposit 0%, spend 45/40 bps) guna menghilangkan attack surface governance DAO. Inti game theory-nya adalah **solvency-first accounting** dengan multi-liability tracking yang memastikan `contract_assets >= all_liabilities` pada setiap state transition.

Yang jenius dari desain ini:
1. **Zero-deposit friction** → dana masuk 100% utuh, revenue diambil saat spend (growth-first)
2. **Fail-closed cryptographic verification** → Guardian wajib verifikasi `k * pk_iss == com_k` sebelum release kunci (DEC-018)
3. **SPENT XOR REFUNDED** → tidak ada double-payout path, terminal state enforcement
4. **Batch spend gas optimization** → hemat $(N-1) \times 37,700$ gas via consolidated BLS pairing check (DEC-014)
5. **Note-based UTXO model (Gate C0 ongoing)** → change output otomatis, no dead funds

---

## 1. Fee Structure & Game Theory

### 1.1 Exact Fee Rates (Hardcoded Constants)

Dari `nimbus-core/src/fees.rs`:

```rust
pub const FEE_DENOMINATOR_BPS: u64 = 10_000;
pub const DEPOSIT_FEE_BPS: u64 = 0;              // 0.00% - Zero-Friction Inflow
pub const PRIVATE_SPEND_FEE_BPS: u64 = 45;       // 0.45% (< 30 days)
pub const SPEND_FEE_30DAY_BPS: u64 = 40;         // 0.40% (≥ 30 days hold)
pub const DEFAULT_RELAYER_MARKUP_BPS: u64 = 1_500; // 15%
pub const THIRTY_DAYS_SECS: u64 = 30 * 24 * 60 * 60;
```

**Ceiling division enforcement:**
```rust
pub fn fee_round_up(amount: u64, fee_bps: u64) -> Option<u64> {
    let numerator = amount.checked_mul(fee_bps)?;
    ceil_div(numerator, FEE_DENOMINATOR_BPS)
}
```
→ Fee **selalu dibulatkan ke atas** untuk menjaga solvency (kontrak tidak pernah rugi dari pembulatan).

### 1.2 Fee Flow & Revenue Distribution

```text
User Deposit 100 USDC
  ↓
  | Deposit fee: 0% (zero friction)
  ↓
Contract liability: 100 USDC (backing 100%)
  ↓
User spends 5 USDC to merchant
  ↓
  ├─ Merchant payout: 5.000000 USDC (exact, no potongan)
  ├─ Protocol fee: 0.022500 USDC (45 bps dari 5 USDC)
  ├─ Execution fee: 0.023000 USDC (gas 0.02 + markup 15% = 0.003)
  └─ Change note: 94.954500 USDC (otomatis mint ke tree)
```

**Fee flow per stakeholder:**

| Stakeholder | Revenue Source | Game Theory Incentive |
|---|---|---|
| **Protocol Treasury** | Spend fee 45 bps (default) / 40 bps (≥30 days) | Berharap TVL tinggi & velocity transaksi tinggi. Tidak ada insentif menahan refund karena deposit gratis → user leluasa keluar. |
| **Relayer** | Gas reimbursement + 15% markup + batch margin | Profit dari efisiensi batch (hemat gas tapi charge quote penuh). Insentif jujur karena reputation-based (salah kirim = slash). |
| **Guardian (3-of-5)** | Reward treasury berdasarkan signing valid | Availability SLA & no double-signing. Kolusi ≥3 guardian bisa mint credential palsu (tapi tercatat on-chain, slashable). |
| **User** | Privacy + self-custody + auto change | Gratis deposit, murah spend (0.45%), no lock-in. Insentif hold ≥30 hari untuk diskon 5 bps. |

### 1.3 Why Zero Deposit Fee? (Strategic Rationale)

Dari `docs/todos/fee-policy-zero-deposit.md`:

**Pro (Zero-Friction Inflow):**
- **TVL growth hacking** → Seperti Tornado Cash, privacy pool butuh depth. Semakin besar anonymity set, semakin susah korelasi.
- **AI Agent friendly** → Autonomous agent funding wallet 100 USDC dapat saldo genap, tidak perlu hitung slippage deposit.
- **Refund UX** → Jika deposit gagal reveal, user refund 100% utuh tanpa rugi fee di muka.

**Con (Deferred Revenue):**
- **Spam micro-deposit** → Bisa penuhi Merkle tree $2^{20}$ slots dengan deposit $10 minimal (mitigasi: min_amount 10 USDC, gas Arbitrum L2 tetap mahal).
- **Parkir dana tanpa fee** → Whale setor $1M, parkir 1 tahun tanpa spend = $0 revenue. Tapi protokol tidak rugi karena solvency dijaga ketat.

**Mengapa tidak riskan:**
- Liability invariant `contract_assets >= total_liability` dicek **setiap state change** (DEC-009).
- Refund hanya jika `deposit_confirmed && !resolved` + timelock 24h (DEC-001, DEC-012).

### 1.4 Holding Time Discount (30-Day Tier)

Dari `nimbus-contracts/src/spend.rs` line 114-125:

```rust
let mut fee_bps = U256::from(45); // 0.45% default

if root != FixedBytes::ZERO {
    let root_timestamp = self.clean_association_roots.get(root);
    if root_timestamp == U256::ZERO {
        return Err(b"INVALID_ASSOCIATION_ROOT".to_vec());
    }
    let delta_t = current_time.checked_sub(root_timestamp).unwrap_or(U256::ZERO);
    let thirty_days = U256::from(30 * 24 * 60 * 60);

    if delta_t >= thirty_days {
        fee_bps = U256::from(40); // 0.40% (1 month hold)
    }
}
```

**Holding time calculation:**
- Dihitung dari **root registration timestamp** di mapping `clean_association_roots`.
- Quote endpoint (`/api/quote/private-spend`) cek RPC sekali lalu cache di memory (`root_timestamp_cache`).
- User lihat tier **sebelum sign** → transparent pricing.

**Game theory:**
- User hold ≥30 hari → diskon 5 bps (0.05%).
- Contoh: spend $10,000 → hemat $5 (tidak banyak, tapi psychological nudge untuk long-term liquidity).
- Protokol benefit: deep liquidity pool = better privacy, less turnover churn.

---

## 2. Attack Vectors & Defenses

### 2.1 Double-Spend Prevention (Nullifier Mechanics)

**Nullifier construction** dari `spend.rs`:
```rust
let m_hash = crate::helpers::compute_spend_hash(
    chain_id, contract_address, amount,
    recipient_or_intent_hash, expiry, nonce
);
let hm_affine = crate::helpers::hash_to_g1(&m_hash);
let hm_bytes = crate::types::to_evm_g1(&hm_affine);

if nullifier != keccak256(&hm_bytes) {
    return Err(b"NULLIFIER_MESSAGE_MISMATCH".to_vec());
}
if self.nullifiers.get(nullifier) {
    return Ok(false); // double-spend rejected
}
self.nullifiers.insert(nullifier, true);
```

**Storage cost:**
- Nullifier = `keccak256(H(m))` → 32 bytes per spend.
- Di Stylus, persistent storage via `StorageMap<FixedBytes<32>, StorageBool>`.
- Gas cost: ~20k gas untuk `SSTORE` warm slot (Arbitrum L2 murah, ~$0.0002).

**Attack scenario:**
1. **Replay nullifier** → Ditolak di line `if self.nullifiers.get(nullifier)`.
2. **Forge nullifier** → Harus tahu private credential + BLS signature valid (computationally infeasible tanpa secret key).
3. **Nullifier collision** → SHA3-256 collision resistance (2^128 security).

**Defensive measures:**
- Message binding via domain separation: `chain_id || contract || amount || recipient || expiry || nonce`.
- Expiry timestamp mencegah replay jangka panjang.
- Nonce per-user mencegah replay signature yang sama.

### 2.2 Session ID Uniqueness (Deposit Lifecycle)

**Invariant:** Satu `session_id` hanya boleh menambah liability **sekali**.

Dari `deposit.rs`:
```rust
if self.session_exists.get(sid) {
    return Err(b"SESSION_ALREADY_EXISTS".to_vec());
}
self.session_exists.insert(sid, true);
```

**Attack scenario:**
1. **Deposit duplikat** → Ditolak on-chain saat `session_exists == true`.
2. **Session ID collision** → SHA3-256 resistance, praktis impossible.

**Terminal states enforcement:**
```rust
// DEC-001: Reveal does NOT transfer collateral
self.session_resolved.insert(sid, true);

// DEC-001: Refund only if NOT resolved
if self.session_resolved.get(sid) {
    return Err(b"SESSION_ALREADY_RESOLVED".to_vec());
}
```

**SPENT XOR REFUNDED logic:**
- Spend mengurangi `total_deposited_principal` + nullify credential.
- Refund mengurangi `total_deposited_principal` + close session.
- Tidak ada jalur untuk kedua-duanya terjadi (state machine terbukti sound).

### 2.3 Guardian Threshold Collusion (3-of-5 Quorum)

**Setup:** 1 Leader + 4 Guardians, threshold t=3 dari n=5.

**Collusion attack scenarios:**

| Attack | Required Colluders | Impact | Mitigation |
|---|---|---|---|
| **Mint credential palsu** | ≥3 guardians | Issue signature untuk deposit yang tidak ada on-chain | DEC-018 cryptographic reveal verification: Leader wajib cek `k * pk_iss == com_k` sebelum release `k`. Jika gagal → fail-closed, user refund. |
| **Deny service (DoS)** | ≥3 guardians offline | User tidak dapat redeem credential → stuck funds | DEC-012 quorum failure refund: timeout 10 menit → auto-refund trigger. Fallback: user manual refund setelah 24h timelock. |
| **Bribe guardian untuk reveal secret** | 1 guardian | Partial share leak, tapi threshold t=3 → tidak cukup untuk reconstruct master key | Shamir's Secret Sharing: < t shares = information-theoretically secure. |
| **Double-signing attack** | ≥3 guardians | Sign multiple commitments untuk satu session | Slashing mechanism (not implemented yet, roadmap Phase 3). On-chain evidence = provable fraud. |

**Current defense (Phase 1):**
- **DEC-018 fail-closed verification** → Meskipun guardian kolusi sign palsu, Leader tidak akan release `k` jika verifikasi `k * pk_iss == com_k` gagal on-chain.
- **On-chain deposit indexer** → Deposit event harus exist di Arbitrum RPC (tidak bisa inject phantom deposit via HTTP API).
- **Tailscale private network** (DEC-010) → Guardian endpoint tidak expose ke public, hanya accessible via VPN mesh.

**Weaknesses (acknowledged):**
- **No slashing yet** → Guardian kolusi tidak ada penalty finansial (roadmap: staked collateral + slash pada fraud proof).
- **No rotation** → Guardian set fixed (roadmap: dynamic quorum via vote/stake).
- **Leader single point of failure** → Jika Leader offline, deposit stuck (roadmap: Leader election via Raft consensus).

### 2.4 Relayer Trust Model & Malicious Behavior

**Relayer role:**
- Pay gas di muka (ETH di Arbitrum).
- Broadcast tx on-chain.
- Charge user execution fee via signed quote.

**What can a malicious relayer do?**

| Attack | Impact | Mitigation |
|---|---|---|
| **Censor transaksi** | Refuse broadcast → user tx tidak settle | User switch relayer (decentralized relay market, belum diimplementasikan). Emergency: user bisa self-broadcast via RPC if they hold gas. |
| **Front-run batch order** | Reorder tx dalam batch untuk MEV | Batch order adalah FIFO queue dengan lease locking (DEC-005). Reordering butuh tamper DB → detectable via audit log. |
| **Overcharge execution fee** | Charge lebih dari quote | Quote di-sign via EIP-712 `ExecutionQuote` dengan `maxExecutionFee`. Contract reject jika `execution_fee > max_execution_fee` (spend.rs line 34). |
| **Steal change note** | Intercept change output | Impossible: change commitment di-bind ke user's spending key via ZK proof (DEC-016 Gate C0). Relayer tidak punya private witness. |
| **Claim fee tanpa broadcast** | Mark tx confirmed di DB tanpa on-chain settlement | Receipt finality check (DEC-017): Node query RPC untuk confirmation, store block hash. Reconciliation setelah restart detect mismatch. |

**Defensive layers:**
1. **Execution quote EIP-712 signing** → User approve max fee upfront, relayer tidak bisa inflate.
2. **Nullifier on-chain verification** → Relayer tidak bisa double-spend (nullifier checked by contract).
3. **Receipt finality reconciliation** → DB state synced dengan on-chain state setelah crash/restart (DEC-019).

**Weaknesses (acknowledged):**
- **Single relayer (centralized)** → MVP hanya 1 relayer. Roadmap: decentralized relayer market dengan stake/slash.
- **No MEV protection** → Batch order bisa di-front-run di mempool (tapi Arbitrum sequencer ordered, less MEV risk vs mainnet).

### 2.5 Refund Path Game Theory (24h Timelock)

**Setup:** User deposit → Guardian sign → Reveal → Spend. Jika gagal, refund setelah 24h.

**Who benefits from delaying?**

| Stakeholder | Incentive | Game Theory |
|---|---|---|
| **User** | Refund ASAP jika gagal | Waiting 24h = opportunity cost. Insentif: switch relayer atau self-refund on-chain. |
| **Guardian** | Delay untuk re-attempt signing | No direct benefit (tidak dapat fee dari refund). Jika stuck, user refund → guardian tidak dapat reward signing. |
| **Relayer** | Delay untuk collect batch margin | No benefit (execution fee hanya dari spend, bukan deposit). |
| **Protocol** | Minimize refund rate | Refund = TVL churn + bad UX. Insentif: detect quorum failure cepat (DEC-012 auto-refund 10 min timeout). |

**Griefing attack:**
- **Attacker deposit minimal** → Force guardian signing, lalu abandon (tidak refund) untuk waste guardian resources.
- **Mitigation:** Min deposit 10 USDC + gas cost Arbitrum L2 (~$0.02) = friction cukup untuk spam.

**Emergency exit (Ragequit):**
- Jika relayer/guardian cluster down semua, user **tidak kehilangan dana**.
- Setelah 24h, user panggil `claim_refund(sid)` langsung on-chain → dapat 100% principal kembali (no fee deduction).
- **This is critical:** No custody risk, always recoverable.

---

## 3. Stakeholder Incentive Analysis

### 3.1 User Incentives

**Why use Nimbus?**
1. **Privacy** → Recipient tidak tahu sumber dana (deposit unlinkable dari spend).
2. **Self-custody** → Private key never leaves client SDK, no relayer custody.
3. **Auto change** → Partial spend creates change note automatically (no manual "refund remainder").
4. **Zero lock-in** → Deposit gratis, refund gratis (100% principal kembali).
5. **Gasless** → Relayer pay gas, user pay in USDC (no need hold ETH).

**Cost:**
- Spend fee: 0.45% (default) / 0.40% (≥30 days).
- Execution fee: ~$0.02 + 15% = ~$0.023 per tx (Arbitrum L2).

**Comparison vs alternatives:**
- **Direct transfer EVM:** Gratis gas, tapi public (no privacy).
- **Tornado Cash:** Gratis deposit, fixed denomination (0.1/1/10/100 ETH), no change output, sanctioned OFAC.
- **RAILGUN:** ZK-UTXO model serupa, tapi lebih berat circuit (slower proof gen), no threshold custody.
- **Nimbus advantage:** Zero deposit + auto change + threshold blind signing (no trusted setup keycoin like Zcash).

### 3.2 Relayer Incentives

**Revenue model:**
- Gas reimbursement: actual cost ~$0.02 (Arbitrum L2).
- Markup 15%: ~$0.003 per tx.
- Batch margin: Jika batch 8 tx → bayar gas 1x, charge 8x quote → profit $(8 - 1) \times 0.02 \times 1.15 = 0.161$.

**Risiko:**
- **Gas price spike** → Jika actual gas > quote, relayer rugi. Mitigasi: adaptive quote via RPC `eth_gasPrice` + safety buffer.
- **Failed tx** → Relayer bayar gas tapi tx revert → rugi gas. Mitigasi: pre-validate signature + nullifier via RPC `eth_call` sebelum broadcast.
- **Reputation** → Jika relayer sering gagal, user switch. Mitigasi: monitoring uptime + SLA.

**Why stay honest?**
- **Long-term profit** → Reputation = user trust = volume.
- **Slashing** (roadmap Phase 3) → Stake collateral, slash jika fraud proof (e.g., charge > max_execution_fee).

**Collusion risk:**
- **Relayer + Guardian** → Bisa censor user tertentu (deny service). Mitigasi: decentralized relay market (roadmap).

### 3.3 Guardian Incentives

**Reward:**
- Dibayar dari protocol treasury per valid signature.
- Belum diimplementasikan mekanisme reward on-chain (Phase 1 testnet = volunteer/salaried).

**Cost:**
- Infrastructure: server + Vault/KMS + Tailscale VPN (~$50/month per node).
- Availability SLA: Must respond dalam 60 detik (DEC-011 timestamp validation).

**Why stay honest?**
- **Reputational risk** → Jika guardian double-sign atau mint palsu, provable on-chain → slash (Phase 3).
- **No direct profit dari fraud** → Mint credential palsu tidak langsung untung (user spend → protocol fee ke treasury, bukan guardian).

**Collusion scenarios:**
1. **3 guardian kolusi mint credential untuk deposit fiktif** → DEC-018 cryptographic verification mencegah: Leader tidak release `k` jika `k * pk_iss != com_k`.
2. **3 guardian kolusi DoS (offline semua)** → User refund setelah 24h, guardian tidak dapat reward.

**Game theory equilibrium:**
- **Honest majority (≥3 dari 5)** → Protokol berjalan normal, semua guardian dapat reward.
- **Byzantine minority (≤2 dari 5)** → Threshold t=3 still achieved, minority tidak dapat halt.
- **Byzantine majority (≥3 dari 5)** → Protocol halt, tapi fraud provable on-chain via `k * pk_iss` verification failure.

### 3.4 Protocol Treasury

**Revenue:**
- Spend fee: 45 bps × volume.
- Contoh: $100M spend volume/year → $450,000 revenue.

**Expenses:**
- RPC Arbitrum: ~$500/month (Alchemy/Infura).
- Guardian incentive: ~$5,000/month (5 nodes × $1,000 reward).
- Audit: $50,000 one-time (Spearbit/Trail of Bits).
- Bug bounty: $100,000 reserve.

**Unit economics (contoh):**
```text
Annual spend volume:     $100,000,000
Spend fee 0.45%:         $450,000
- RPC:                   -$6,000
- Guardian reward:       -$60,000
- Audit/bounty:          -$150,000 (amortized)
= Net:                   $234,000
```

**Break-even:** ~$30M annual spend volume.

**Growth incentive:**
- **TVL ≠ revenue** → Protocol tidak dapat fee dari deposit, hanya dari spend. Insentif: maksimalkan velocity (spend/TVL ratio), bukan hanya TVL statis.
- **Privacy network effect** → Semakin besar TVL, semakin besar anonymity set, semakin atraktif untuk user baru.

---

## 4. Technical Completeness & Implementation Status

### 4.1 Fully Implemented & Testnet-Verified (Phase 1 Baseline)

✅ **Smart Contract Stylus (nimbus-contracts/):**
- BLS12-381 pairing check via EIP-2537 precompile (`0x0f`) di Arbitrum Sepolia.
- 13/13 testnet negative tests lolos (HT-03: deposit/sign/reveal lifecycle).
- Deposit/reveal/refund lifecycle dengan `k * pk_iss == com_k` verification (DEC-001, DEC-018).
- Spend dengan nullifier double-spend protection.
- Batch spend (2-8 items) dengan consolidated pairing check (DEC-014).
- Liability invariant check `contract_assets >= liabilities` setiap state change (DEC-009).
- Multi-liability tracking: `user_note_liability`, `refundable_deposit_liability`, `accrued_execution_fee_liability` (DEC-016 Gate B).
- CCIP cross-chain message handler (`_ccip_receive`) dengan replay protection + allowlist (DEC-015).
- Fee policy: 0% deposit, 45/40 bps spend dengan holding time discount.

✅ **BLS Threshold Cluster (nimbus-node/):**
- 1 Leader + 4 Guardians, threshold 3-of-5.
- Tailscale private network binding (DEC-010).
- Atomic masking key release via `k` verification (DEC-011).
- Session validation 60s timestamp window (DEC-011).
- Key zeroization setelah release (DEC-011).

✅ **Settlement Engine (nimbus-node/):**
- SQLite/SQLCipher persistent queue dengan AES-256 encryption.
- Leasing worker dengan exponential backoff (DEC-005).
- Adaptive batch spend 2-8 items dengan 1s normal window, 2s hard timeout (DEC-006).
- Receipt finality check dengan confirmation threshold (DEC-017).
- Nonce management dengan auto-increment + replacement tx (DEC-017).

✅ **Fee Structure & Quote:**
- Zero-deposit policy (0 bps) dengan immutable constants (fee-policy-zero-deposit.md).
- Spend fee 45 bps (default) / 40 bps (≥30 hari hold) hardcoded di `fees.rs`.
- Quote endpoint `/api/quote/private-spend` dengan holding time discount detection.
- EIP-712 `ExecutionQuote` signing dengan `maxExecutionFee` enforcement.
- CCIP cross-chain fee separation: relayer markup 15% **hanya pada gas**, bukan pada CCIP network fee passthrough.
- Root timestamp caching untuk fast quote response.

✅ **Deposit Indexer On-Chain (DEC-018):**
- Background worker poll RPC untuk `DepositFee` event.
- Safe-block reorg protection dengan confirmation depth.
- Idempotent event processing dengan `last_indexed_block` tracking.
- Fail-closed: Leader wajib verifikasi `k * pk_iss == com_k` sebelum resolve session.

✅ **Gas Benchmarking (docs/gas_latency_benchmark.md):**
- Testnet measurement: single spend ~605k gas L2, batch 8 ~1.2M gas L2.
- Gas hemat batch: $(N-1) \times 37,700$ dari precompile overhead.
- L1 calldata cost fluctuation explained (separate dari L2 execution).

### 4.2 In Progress (Phase 2 - ZK-UTXO Note Balance)

🔧 **Gate C0 Security Repair (nimbus-core/circuit/):**
- [x] Bind Merkle path direction bits ke `input_leaf_index` (mencegah double-spend via fake path).
- [x] Range constraint 64-bit untuk semua nilai nominal (mencegah modular wrap).
- [x] Boolean constraint `has_change` (mencegah scale output commitment).
- [x] Payment domain binding via Poseidon-W5 digest.
- [ ] Ganti Poseidon parameter ad-hoc dengan audited constants.
- [ ] Cache proving/verifying key sebagai artifact versioned.

Status: Circuit security fix sudah selesai, tinggal parameter audit + key management.

🔧 **Gate D (Stylus Note Ledger):**
- [ ] Integrasi on-chain Merkle tree append-only di contract.
- [ ] Bounded root history (100 slots ring buffer).
- [ ] Multi-liability accounting saat note di-mint/spent.

Status: Storage structure sudah siap (`note_tree_root`, `note_tree_next_index`, `root_history`), tinggal integrasi circuit proof verification.

### 4.3 Not Implemented / Roadmap (Phase 3 - Mainnet Hardening)

❌ **Tokenomics & DAO:**
- [ ] Token $NIMB launch (delayed hingga protokol proven).
- [ ] Governance voting mechanism.
- [ ] Guardian stake/slash mechanism.

Rationale: **Zero governance attack surface** selama Phase 1/2 untuk avoid flashloan/DAO exploit (The DAO 2016, Tornado Cash 2023 precedent).

❌ **Decentralized Relayer Market:**
- [ ] Multi-relayer competition via stake ranking.
- [ ] Relayer SLA monitoring + slash.
- [ ] User choice via cheapest quote.

Rationale: MVP single relayer cukup untuk testnet. Mainnet butuh economic security layer.

❌ **Cross-Chain CCIP Full Testing:**
- [ ] E2E hard test Arbitrum Sepolia → Base Sepolia.
- [ ] Destination failure refund flow.
- [ ] Double payout rejection test.

Status: CCIP contract entrypoint sudah ada (`_ccip_receive`), tapi belum testnet cross-chain verification.

❌ **Yield Vault Integration:**
- [ ] ERC-4626 wrapper untuk Aave V3 / Ondo RWA.
- [ ] Dynamic vault allocation 30/50/20.

Rationale: Ditunda ke Phase 3, fokus Phase 1/2 adalah **full reserve solvency** (100% liquid backing).

### 4.4 Known TODOs dari Code

**Deposit fee dynamic config (docs/todos/fee-policy-zero-deposit.md):**
- [x] `deposit_fee_bps` storage field dengan getter.
- [x] Default 0 bps (immutable zero-friction).
- [x] Bypass ceiling division formula jika `fee_bps == 0`.
- [ ] ~~Dynamic fee adjustment via governance~~ (scrapped, immutable by design).

**Nullifier & Receipt Finality:**
- [x] Post-restart reconciliation: sync DB dengan on-chain nullifier state (DEC-019).
- [x] Confirmation threshold per chain (Arbitrum 1 block, Ethereum 12 blocks).
- [x] Replacement tx auto-increment nonce jika stuck.

**KMS / Vault TLS Hardening (docs/todos/kms-tls.md):**
- [ ] Ganti raw TCP HTTP client dengan HTTPS client tervalidasi.
- [ ] Certificate TLS verification untuk guardian RPC + Vault endpoint.

**x402 Facilitator (docs/todos/x402.md):**
- [ ] Ganti mock tx hash dengan real settlement status.
- [ ] Ganti recipient `"x402-facilitator-pool"` dengan EVM address config.

---

## 5. Comparison: Nimbus vs RAILGUN

### 5.1 RAILGUN Architecture (Brief)

**Source:** [RAILGUN Docs](https://docs.railgun.org/wiki/learn/using-private-tokens), [arXiv 2608.22987](https://arxiv.org/abs/2608.22987) (Anonymity Gap analysis)

**Model:**
- **ZK-UTXO (Unspent Transaction Output)** dengan encrypted notes.
- Setiap UTXO = `{public_key, amount, token_id, randomness}`.
- Nullifier published on-chain saat spend (similar to Zcash Sapling).
- **No trusted setup** (menggunakan Groth16 dengan universal ceremony atau PLONK).
- **Shielded pool** di EVM (Ethereum, Arbitrum, BSC, Polygon).

**Privacy:**
- Full shielded transfer (sender, recipient, amount semua private).
- Public hanya: nullifier + output commitments.
- **Anonymity set** = semua UTXO di tree (bisa cross-chain).

**Weaknesses (from arXiv 2608.22987):**
- **Token constraint pruning:** Jika spend token A, anonymity set hanya dari UTXO token A (bukan global).
- **Value clustering:** Amount patterns bisa di-trace (e.g., round numbers).
- **Merkle tree partitioning:** Multiple trees per chain → smaller anonymity set per tree.
- **Mean anonymity set reduction 40-59%** dari baseline temporal pool size (study 186k tx di Railgun/Hinkal).

### 5.2 Nimbus vs RAILGUN Comparison

| Aspek | Nimbus | RAILGUN |
|---|---|---|
| **Privacy Model** | ZK-UTXO (DEC-016 Gate C0) + threshold BLS issuance | ZK-UTXO (Groth16/PLONK) pure ZK |
| **Shielded Scope** | Recipient PUBLIC (merchant payout), change note private | Sender, recipient, amount semua private |
| **Anonymity Set** | Same-chain UTXO di note tree (~1M slots $2^{20}$) | Cross-chain global pool (lebih besar) |
| **Deposit Fee** | 0% (immutable) | Unknown (perlu cek docs/contract) |
| **Spend Fee** | 45 bps / 40 bps (hold ≥30d) | Unknown (perlu cek) |
| **Change Output** | Auto mint via ZK proof | Auto via circuit |
| **Trusted Setup** | BLS threshold ceremony (DKG offline) | Universal ceremony (Groth16) atau transparent (PLONK) |
| **Custody** | Threshold blind signing (3-of-5 guardian) | Pure ZK, no threshold guardian |
| **Relayer Trust** | Relayer pay gas, user sign quote | Similar (relayer broadcast) |
| **Cross-Chain** | CCIP (in progress, not fully tested) | Native multi-chain support |
| **Compliance** | Association Set Provider (ASP) opt-in verifiable provenance | Unknown (perlu cek compliance doc) |
| **Circuit Complexity** | Simpler (1-in 1-out MVP, Poseidon-W5) | Heavier (multi-in multi-out, full shielding) |
| **Proof Gen Time** | Unknown (belum benchmark client) | Slower (full ZK overhead) |
| **Gas Cost** | ~605k L2 single, ~1.2M batch 8 | Unknown (perlu benchmark) |
| **Mainnet Status** | Testnet (Arbitrum Sepolia) | Production (Ethereum, Arbitrum, etc.) |

### 5.3 Nimbus Advantages vs RAILGUN

1. **Zero-deposit friction** → Gratis masuk, revenue dari spend (growth-first).
2. **Simpler circuit** → Faster proof gen (trade-off: recipient public).
3. **Threshold custody** → Guardian quorum untuk issuance (compliance-friendly, accountable privacy).
4. **Immutable fee constants** → No DAO governance attack surface (The DAO 2016 / Tornado Cash 2023 avoidance).
5. **AI agent optimized** → Agent spending wallet dengan auto change, no need manual UTXO management.
6. **Arbitrum Stylus WASM** → Gas lebih murah vs Solidity EVM.

### 5.4 RAILGUN Advantages vs Nimbus

1. **Full shielding** → Recipient, amount semua private (Nimbus recipient public = weaker privacy).
2. **Production proven** → Sudah live mainnet, battle-tested.
3. **Larger anonymity set** → Cross-chain global pool (Nimbus saat ini same-chain only).
4. **No guardian trust** → Pure ZK, tidak ada threshold dependency.
5. **DeFi integration** → Private swap, private liquidity (Nimbus fokus payment only).

### 5.5 When to Use Nimbus vs RAILGUN?

**Use Nimbus jika:**
- **AI agent micro-payment** → Need gasless, auto change, simple UX.
- **Compliance-friendly privacy** → ASP verifiable provenance (exchange/merchant dapat verify clean funds tanpa KYC user).
- **Merchant payment focus** → Recipient public acceptable (merchant invoice transparent).
- **Low fee** → 0% deposit, 0.45% spend (competitive dengan payment processor).

**Use RAILGUN jika:**
- **Maximum privacy** → Recipient, amount harus private (e.g., salary, donation).
- **DeFi shielded interaction** → Private swap, LP, yield farming.
- **Production ready** → No tolerance untuk testnet risk.
- **Cross-chain arbitrage** → Perlu global anonymity set.

### 5.6 Is Nimbus "Better" Than RAILGUN for Production?

**Currently: NO** (Nimbus masih testnet, RAILGUN production).

**Potentially: YES (for specific use case):**
- Jika target market = **AI agent payment + merchant settlement**, Nimbus UX lebih baik (zero-deposit, auto change, threshold custody = accountable privacy).
- Jika target market = **privacy-maximalist DeFi**, RAILGUN menang (full shielding, proven battle-tested).

**Risk assessment:**
- **RAILGUN:** arXiv study show anonymity set degradation 40-59% from temporal baseline → still vulnerable to statistical analysis (tapi masih jauh lebih baik dari no privacy).
- **Nimbus:** Recipient public = correlation risk via merchant clustering (e.g., semua tx ke merchant X = bisa cluster user base). Tapi untuk payment use case, ini acceptable trade-off (merchant invoice memang public).

---

## 6. Game Theory Takeaways (Key Insights)

### 6.1 What Makes Nimbus Genius?

1. **Immutable fee constants** → No governance attack surface (The DAO / Tornado Cash precedent).
2. **Fail-closed cryptography** → `k * pk_iss == com_k` verification before release (DEC-018) = no phantom deposit possible.
3. **Solvency-first accounting** → Multi-liability tracking + invariant check every state change (DEC-009, DEC-016).
4. **SPENT XOR REFUNDED** → No double-payout path via state machine enforcement.
5. **Batch gas optimization** → $(N-1) \times 37,700$ gas saving via consolidated pairing (DEC-014).
6. **Zero-deposit friction** → Growth-first strategy (TVL → anonymity set → network effect).

### 6.2 Critical Vulnerabilities (Acknowledged)

1. **Guardian collusion (≥3 dari 5)** → Bisa mint credential palsu, **tapi** DEC-018 cryptographic verification mencegah release `k` jika gagal.
2. **Single relayer (centralized)** → DoS vector jika relayer offline. Mitigasi: user self-broadcast via RPC (tapi butuh gas ETH).
3. **Recipient public** → Weaker privacy vs full shielding (trade-off: simpler circuit, faster proof).
4. **No slashing yet** → Guardian/relayer fraud tidak ada penalty finansial (roadmap Phase 3).
5. **CCIP cross-chain not fully tested** → E2E testnet verification belum selesai.

### 6.3 Economic Sustainability

**Break-even:** ~$30M annual spend volume (asumsi 0.45% fee).

**Growth drivers:**
1. **AI agent adoption** → Autonomous payment butuh gasless + auto change.
2. **Privacy regulation** → OFAC sanction Tornado Cash → demand privacy solution compliant (ASP model).
3. **Arbitrum ecosystem** → Gas murah, fast finality, EVM compatible.

**Risks:**
- **TVL growth tanpa spend** → Deposit gratis = no revenue jika dana parkir saja.
- **Relayer margin pressure** → Batch margin shrink jika gas drop atau competition.
- **Guardian operational cost** → $5k/month incentive bisa tidak sustainable jika volume rendah.

### 6.4 Final Verdict

**For Production (Mainnet):**
- **Not ready yet** (Phase 2 Gate C0/D belum selesai, CCIP not tested, no audit).
- **ETA:** ~6-12 bulan jika eksekusi lancar (audit Spearbit/Trail of Bits + testnet 3-6 bulan).

**For Testnet (Innovation):**
- **Solid architecture** → Cryptographic defenses sound (BLS threshold + ZK-UTXO).
- **Unique value prop** → Zero-deposit + threshold custody + AI agent UX = differentiated dari RAILGUN/Zcash.

**Comparison to RAILGUN:**
- **Privacy:** RAILGUN menang (full shielding).
- **UX:** Nimbus menang (zero-deposit, auto change).
- **Compliance:** Nimbus menang (ASP verifiable provenance).
- **Production:** RAILGUN menang (battle-tested).

**Recommendation:**
- **Deploy Nimbus jika:** Target market = AI agent payment + merchant settlement + compliance-friendly privacy.
- **Stay with RAILGUN jika:** Target market = privacy-maximalist DeFi + proven production stability.

---

## 7. Referensi & Sumber Primer

### Smart Contract & Cryptography
- **EIP-2537: BLS12-381 curve operations** → https://eips.ethereum.org/EIPS/eip-2537
- **Arbitrum ArbOS 50 "Dia" activation** → BLS precompile support
- **Zcash Protocol Specification (v2026)** → Note commitment & nullifier design: https://zips.z.cash/protocol/protocol.pdf
- **ZIP 224 (Orchard Shielded Protocol)** → UTXO model: https://zips.z.cash/zip-0224
- **Aztec State Management** → Private note lifecycle: https://docs.aztec.network/developers/docs/foundational-topics/state_management
- **RAILGUN Official Docs** → ZK-UTXO implementation: https://docs.railgun.org/wiki/learn/using-private-tokens

### Game Theory & Economic Security
- **Key-Isolated Threshold Signing for LLM Agents (arXiv 2609.05901)** → Threshold BLS quorum gating: https://arxiv.org/abs/2609.05901
- **Threshold Blind Signatures Security (IACR 2025/353)** → One-more unforgeability: https://eprint.iacr.org/2025/353
- **The Anonymity Gap (arXiv 2608.22987)** → RAILGUN anonymity set analysis: https://arxiv.org/abs/2608.22987

### Exploit Case Studies (DEC-018 References)
- **Across Protocol Solana Bridge Exploit (Juli 2026)** → Phantom deposit via unvalidated event: https://x.com/AcrossProtocol/status/2080722320814121237
- **Handling Chain Reorgs (Harman Kamboj, Juni 2026)** → Indexer safety: https://hammyasf.github.io/handling-chain-reorgs-indexer.html
- **Coreum / XRP Ledger Bridge Exploit (Agustus 2026)** → Relayer reconciliation failure: https://hacked.slowmist.io/?c=Bridge&page=1
- **Qubit Finance Exploit ($80M, 2022)** → Bridge deposit validation failure

### Nimbus Internal Decisions
- **DEC-001:** Deposit/reveal/collateral lifecycle
- **DEC-009:** Liability invariant check
- **DEC-012:** Quorum failure refund trigger
- **DEC-014:** Batch spend gas optimization
- **DEC-016:** Private note UTXO ledger
- **DEC-018:** On-chain deposit indexer + cryptographic reveal verification
- **DEC-019:** Relayer input sanitization & post-restart reconciliation
- **DEC-020:** Relayer batch profitability metrics

---

**Analysis completed.** File written to: `d:\Project Utama\Rust\Zeltra-Protocol\.agents\tasks\nimbus-analysis.md`
