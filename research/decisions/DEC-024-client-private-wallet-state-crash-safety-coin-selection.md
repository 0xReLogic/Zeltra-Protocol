# DEC-024: Client Private Note Wallet State, Crash-Safe UTXO Lifecycle, and Privacy-Preserving Coin Selection

**Status:** Accepted  
**Date:** October 2026  
**Applies to:** `nimbus-sdk/src/wallet/note_wallet.rs`, `nimbus-sdk/src/x402/pool.rs`, `nimbus-sdk/src/lib.rs`  
**Related DECs:** DEC-016 (ZK-UTXO Model), DEC-016A (Nullifier & Circuit Invariants), DEC-016B (Gate C0 & Range Bounds), DEC-022 (Boundary Gaps), DEC-023 (Siloed Nullifiers)

---

## 1. Context & Vulnerability Vectors

While Gate C (ZK Circuit) and Gate D (Stylus Verifier & Settlement) mathematically enforce that zero-knowledge proofs are sound and double-spending is prohibited on-chain, **Gate E (Client Wallet State)** is where user funds and transaction privacy are most vulnerable to operational and operational-security failures:

1. **Deanonymization via Wallet Fingerprinting & Subset-Sum Attacks:**
   - Academic empirical studies (ACM CCS 2024/2025: *Attacking Anonymity Set in Tornado Cash via Wallet Fingerprints*, arXiv:2508.19218: *The Subset Sum Matching Problem*) demonstrated that over 80% of privacy pool users are deanonymized not through circuit cryptanalysis, but through **client-side coin selection heuristics** and deterministic change note amounts.
   - Naive knapsack selection allows external observers to correlate deposit denominations and change note creation across multiple hops.
2. **The "Ghost Note" & Lockout Race Conditions (Zcash & Aztec PXE Post-Mortems):**
   - If a client wallet eagerly marks an input note as `Spent` upon proof submission to a relayer, a dropped transaction, network partition, or mempool eviction results in permanent loss of funds from the user's perspective (**Ghost Note syndrome**).
   - Conversely, if the note remains marked `Unspent`, concurrent requests (or multiple browser tabs) will attempt to spend the same note, resulting in on-chain reverts (`NullifierAlreadyUsed`), wasted execution fees, and poisoned relayer queues.
3. **Compositional Failures in Hybrid Ecash/ZK Systems (IACR ePrint 2026/2174):**
   - As proven in *Blind Spots in Blind Signatures* (Chen et al., Sept 2026), systems combining blind signatures (BDHKE vouchers) with persistent state frequently suffer from state desynchronization between issuance, local recovery, and on-chain spending.
4. **Wasted Client Proof Cycles on Expired Roots:**
   - Groth16 proof generation in browser WebAssembly takes 1–3 seconds of heavy CPU and memory. Proving against an on-chain Merkle root that has already fallen outside the contract's 64-slot `accepted_note_roots` ring buffer causes immediate on-chain revert.

---

## 2. Decision & Architecture for Nimbus Protocol

To resolve these vulnerabilities, `nimbus-sdk` transitions from the legacy `AgentTokenPool` (rigid fixed-denomination BDHKE vouchers) to an audited **`PrivateNoteWallet`** implementing the following architectural principles:

### A. 4-Stage UTXO Lifecycle State Machine with Two-Phase Commit (2PC)

Every note tracked by the client wallet exists in one of four strictly enforced lifecycle states:

$$\text{Unconfirmed} \longrightarrow \text{Unspent} \overset{\text{Reserve}}{\underset{\text{Rollback}}{\rightleftharpoons}} \text{Reserved} \longrightarrow \text{Spent}$$

1. **`Unconfirmed`:** A newly minted change note whose commitment has been computed locally and submitted to the relayer, but whose transaction receipt is not yet finalized on-chain.
2. **`Unspent`:** A fully confirmed note verified against the contract's Merkle tree, available for coin selection.
3. **`Reserved(reservation_id, timeout_timestamp)`:** An unspent note locked for an active spending session.
   - Protected by a lease timeout (`RESERVATION_TTL_SECS = 120`).
   - Multiple concurrent tabs or processes cannot acquire a lease on the same note.
4. **`Spent`:** Permanently spent note whose nullifier has been settled on-chain.

#### Two-Phase Commit (Crash-Safety Algorithm)
Before transmitting a transaction to the relayer:
1. **Phase 1 (Pre-Commit to Persistent Encrypted Storage):**
   - Lock input note: `status = Reserved(session_id, now + 120s)`.
   - Persist speculative change note: `status = Unconfirmed(session_id)`.
   - Flush state to encrypted local storage (`localStorage` / IndexedDB / AES-GCM encrypted keystore).
2. **Execution:** Generate Groth16 proof and transmit payload to relayer.
3. **Phase 2 (Post-Commit / Finalization):**
   - On receipt success: Input note transition $\rightarrow$ `Spent`; Change note transition $\rightarrow$ `Unspent`.
   - On explicit failure or timeout ($t > \text{timeout}$ and on-chain nullifier check returns false): Input note rolled back $\rightarrow$ `Unspent`; Change note discarded $\rightarrow$ `Aborted`.

### B. Privacy-Preserving Stochastic Coin Selection

Inspired by Aztec's `balance-set.nr` (`preprocess_notes_min_sum`) and WabiSabi variable-amount privacy mechanisms:
1. **Total Spend Requirement:**
   $$\text{Required} = \text{Payout Amount} + \text{Protocol Fee} + \text{Max Execution Fee}$$
2. **Selection Strategy:**
   - **Exact Match First:** If an unspent note exactly equals $\text{Required}$ (within dust tolerance), select it directly to produce **Zero Change Note** (`has_change = 0`), eliminating change linkability.
   - **Single Minimum Sufficient Note (Best Fit):** Find the smallest note $N$ such that $\text{value}(N) \ge \text{Required}$. This minimizes the value of the resulting change note and avoids fragmenting wallet balance.
   - **Stochastic Tie-Breaking:** If multiple candidate notes have equal value or fall within a privacy bucket ($\pm 5\%$), randomly select among them using secure cryptographic RNG to eliminate deterministic fingerprinting.
   - **Anti-Dust Guard:** If $\text{Change} = \text{value}(N) - \text{Required} < \text{DUST\_THRESHOLD}$ (e.g. $< 1{,}000$ base units / $0.001 USDC$), the change is either added to the execution fee or merged into a consolidation spend, preventing useless unspendable micro-UTXOs.

### C. Pre-flight Merkle Root Freshness Guard

Before invoking `nimbus-core::generate_note_proof`:
1. The SDK queries the smart contract's latest `merkle_root` or validates that `accepted_note_roots(note.merkle_root) > 0`.
2. If the note's recorded root has expired from the circular buffer, the SDK automatically performs a lightweight local Merkle witness update against the latest tree root without requiring user interaction.

### D. Single Seed BIP-32 / EIP-2333 Key Derivation

Following Paper 2026/1621 (*Z-SCAPE*) and Paper 2026/513 (*zkBSA*):
- Master Seed (256-bit secure entropy) derives:
  - $\text{sk}_{\text{spend}} \in \mathbb{F}_r$: Private spending key (never leaves client device).
  - $\text{pk}_{\text{owner}} = \text{Poseidon}(\text{sk}_{\text{spend}}, \text{DOMAIN\_OWNER})$: Public owner identifier for note commitments.
  - $\text{nk} = \text{Poseidon}(\text{sk}_{\text{spend}}, \text{DOMAIN\_NULLIFIER\_KEY})$: Nullifier derivation key.
  - $\text{vk}_{\text{view}}$: Encryption view key for note recovery and oblivious discovery.

---

## 3. Implementation Blueprint for `nimbus-sdk`

- **Module Location:** `nimbus-sdk/src/wallet/mod.rs` & `nimbus-sdk/src/wallet/note_wallet.rs`
- **Exposed Client Methods:**
  - `PrivateNoteWallet::new(seed: [u8; 32]) -> Self`
  - `wallet.balance() -> u64` (Aggregate balance of all `Unspent` notes)
  - `wallet.select_note_for_spend(amount: u64, protocol_fee: u64, exec_fee: u64) -> Result<SelectedSpend, WalletError>`
  - `wallet.prepare_spend_proof(...) -> Result<SpendProofPayload, WalletError>`
  - `wallet.commit_spend_receipt(session_id: &str, tx_hash: &str) -> Result<(), WalletError>`
  - `wallet.rollback_spend(session_id: &str) -> Result<(), WalletError>`
  - `wallet.export_encrypted_backup(password: &str) -> Result<String, WalletError>`

---

## 4. Verification & Testing Gate

1. **Deterministic State Tests:** Verify all state transitions (`Unconfirmed` $\rightarrow$ `Unspent` $\rightarrow$ `Reserved` $\rightarrow$ `Spent` $\rightarrow$ `Rollback`).
2. **Concurrency / Double-Reservation Protection:** Ensure a second spend attempt while a note is `Reserved` returns `Error::NoteAlreadyReserved`.
3. **Exact Change Conservation:** Ensure $\text{Input} = \text{Merchant} + \text{Protocol Fee} + \text{Exec Fee} + \text{Change}$ holds strictly for all coin selection branches.
4. **Zero-Change Mode:** Ensure that when $\text{Input} == \text{Required}$, `has_change == 0` and no change commitment is generated.

---

## 5. Academic & Literature References

1. **Compositional Failures in Chaumian Ecash & Wallet Recovery:**
   Huaifeng Chen, Yuchang Zhang, Yu Cheng. *"Blind Spots in Blind Signatures: A System-Level Security Analysis of Deployed Chaumian Ecash"*. IACR ePrint 2026/2174 (Sept 2026).
   Local Artifact: [`jurnal/pdf/2026-2174.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-2174.pdf).
2. **Self-Custodial Key Protection under Entropy Failures:**
   Mehmet Sabir Kiraz, Suleyman Kardas. *"Z-SCAPE: Zero-Knowledge Self-Custodial Credential Operation for Privacy-Preserving Asset Protection under Entropy-Source Failure"*. IACR ePrint 2026/1621 (Aug 2026).
   Local Artifact: [`jurnal/pdf/2026-1621.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-1621.pdf).
3. **Auditable & Compliant Stealth Addresses:**
   Siyuan Zheng, Zhe Han. *"zkBSA: Auditable and Compliant Stealth Addresses for Blockchains"*. IACR ePrint 2026/513 (March 2026).
   Local Artifact: [`jurnal/pdf/2026-513.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-513.pdf).
4. **Wallet Fingerprinting & Anonymity Degradation:**
   ACM CCS 2024/2025. *"Attacking Anonymity Set in Tornado Cash via Wallet Fingerprints"*. DOI: 10.1145/3672608.3707896.

