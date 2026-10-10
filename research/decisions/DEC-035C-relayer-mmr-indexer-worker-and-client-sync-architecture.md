# DEC-035C: Relayer MMR Indexer Worker & Client Sync Synchronization Architecture

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md))
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-node/src/mmr_indexer.rs` (Background daemon indexing on-chain commitments)
  - `nimbus-node/src/routes.rs` & `nimbus-node/src/handlers/mmr.rs` (`GET /api/v1/mmr/proof/{leaf_index}` & `GET /api/v1/mmr/tip`)
  - `nimbus-node/src/db/schema.rs` (SQLite schema `mmr_leaves` and persistent peak snapshots `mmr_state`)
  - `nimbus-sdk/src/wallet/note_wallet.rs` (Dynamic MMR inclusion proof sync client & witness builder)
  - `nimbus-contracts/src/merkle.rs` (Stylus on-chain `NoteCommitmentAppended` event emissions)
  - `nimbus-core/src/note.rs` (`MerkleMountainRange` accumulator, peak bagging, and proof types)
- **Parent & Related DECs:**
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Master Zero-Tech-Debt Blueprint)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) (Merkle Mountain Range Commitment Accumulator)
  - [`DEC-018`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-018-on-chain-deposit-indexer-and-cryptographic-reveal-verification.md) (Deposit Indexer Architecture & Reorg Protection)
  - [`DEC-024`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-024-client-private-wallet-state-crash-safety-coin-selection.md) (Client Private Wallet State & Crash Safety)

---

## 1. Executive Summary & Problem Statement

In [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md), Zeltra Protocol successfully eliminated the static $2^{20}$ capacity wall of LeanIMT by designing a **Merkle Mountain Range (MMR)** accumulator in the Stylus WASM contract (`nimbus-contracts/src/merkle.rs`) and circuit constraint system (`nimbus-core/src/note.rs`). However, the cross-agent review in [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Issue #5) revealed a critical integration void:

```
        THE STATUS-QUO INTEGRATION GAP (RESOLVED BY DEC-035C)
┌─────────────────────────┐               ┌─────────────────────────┐
│ Smart Contract (Stylus) │               │   Client Wallet (SDK)   │
│  - Appends commitments  │               │  - Holds private note   │
│  - Emits on-chain event │               │  - Needs MMR siblings   │
│  - Stores O(log N) peaks│               │  - Fills zeros locally! │
└────────────┬────────────┘               └────────────▲────────────┘
             │                                         │
             ▼                                         │
   NoteCommitmentAppended                              │
        Event Log                                      │  NO SYNC ENDPOINT!
             │                                         │  (Multi-wallet spends
             ▼                                         │   are blocked)
    ┌─────────────────┐                                │
    │   nimbus-node   │ ═══════════════════════════════╛
    │ (Relayer Node)  │  NO MMR INDEXER BACKGROUND WORKER!
    └─────────────────┘
```

1. **The Missing Relayer Pipeline:**
   While the smart contract emits `NoteCommitmentAppended(leaf_index, commitment, bagged_root)`, `nimbus-node` had no background event listener to index these commitments into persistent storage.
2. **The Client Proof Blocker:**
   A client wallet generating a Groth16 spend proof needs:
   - The mountain peak containing its leaf.
   - Sibling hashes along the mountain's Merkle path.
   - Sibling peak hashes for folding into the bagged root.
   - The current on-chain `leaf_count`.
   Without an authoritative relayer sync API, clients could only prove inclusion if they had observed every transaction locally from genesis. In a multi-user, multi-wallet ecosystem, second-party spends were impossible.
3. **The Founder's Zero-Tech-Debt Directive:**
   Gate G (Arbitrum Sepolia Testnet) cannot be cleared with mock client witnesses. Zeltra mandates a fully automated, reorg-safe, privacy-preserving MMR synchronization pipeline connecting **Smart Contract $\to$ Relayer Indexer $\to$ REST API $\to$ Client SDK**.

---

## 2. Post-Mortem & Vulnerability Archaeology

The architecture of Zeltra's MMR indexer and sync API directly resolves real-world vulnerabilities identified in blockchain indexers and ZK light-client synchronization.

```
       MMR INCLUSION PROOF SYNC: PRIVACY-PRESERVING DATA PLANE
┌────────────────────────────────────────────────────────────────────────┐
│ Client Wallet                                            Relayer Node  │
│                                                                        │
│ 1. Client identifies unspent Note at leaf_index = 42                   │
│                                                                        │
│ 2. Queries by public integer position (ZERO PRIVACY LEAK):             │
│    GET /api/v1/mmr/proof/42 ───────────────────────────► Check bounds  │
│    (Never reveals note commitment or nullifier!)         42 < leaf_cnt │
│                                                               │        │
│ 3. Receives authenticated siblings & peaks ◄──────────────────┘        │
│                                                                        │
│ 4. Verifies locally: bag_peaks(peaks) == contract.bagged_root          │
│ 5. Synthesizes Groth16 witness for PrivateNoteCircuit                  │
└────────────────────────────────────────────────────────────────────────┘
```

### A. The Hyperbridge MMR Verifier Exploit (2026) — Out-of-Bounds Leaf Skipping
- **Vulnerability Context:** On April 13, 2026, the Hyperbridge cross-chain protocol suffered an exploit resulting in $237,000 drained and 1 billion unauthorized tokens minted ([Post-mortem, 2026](https://github.com/hyperbridge/core)). The bug affected production MMR verifiers across `solidity-merkle-trees#51` and `pallet-beefy-mmr`.
- **Root Cause:** The MMR verification function accepted an array of leaves and iterated through peaks. However, it **failed to check that all provided leaves were consumed** (*iterator exhaustion failure*). A malicious prover submitted a legitimate leaf at index 0 and a forged leaf at `leaf_index = 1` with `leaf_count = 1`. The verifier processed the first leaf, silently skipped the out-of-bounds second leaf, and returned `true`.
- **Mitigation in DEC-035C:**
  1. The relayer sync endpoint strictly validates that the requested index satisfies:
     $$\text{leaf\_index} < \text{current\_leaf\_count}$$
     Any query with $\text{leaf\_index} \ge \text{current\_leaf\_count}$ immediately returns `404 Not Found` (`LeafIndexOutOfBounds`).
  2. Single-note UTXO verification: Zeltra's circuit and indexer enforce single-leaf proofs with exact mathematical bounds checking, eliminating multi-leaf iterator skip vulnerabilities.

### B. Privacy Boundary Leaks in ZK Light Client Indexers (ZIP-314, Zinder, RIME)
- **Vulnerability Context:** In Zcash and privacy protocol research ([ZIP-314](https://zips.z.cash/); [Zinder Architecture](https://github.com/gustavovalverde/zinder); [RIME Client](https://github.com/tanctl/rime)), exposing an API where light wallets query state by sensitive identifiers (e.g. note commitments, viewing keys, or transaction hashes) constitutes a catastrophic privacy leak. The server operator can deanonymize the client, cluster their UTXOs, and link incoming payments to outbound spends.
- **Privacy Boundary in Zeltra:**
  - The client queries `GET /api/v1/mmr/proof/{leaf_index}` using **only the integer leaf index** in the public tree (e.g. index 42).
  - The client NEVER transmits note secrets, commitment preimages, or nullifiers to the relayer.
  - The integer index is public on-chain metadata (all leaves are sequentially numbered from $0$ to $N-1$). Querying by integer position leaks zero information regarding note ownership.

### C. Reorg Desyncs in Layer-2 Rollup Indexers (Arbitrum Nitro)
- **Vulnerability Context:** In Arbitrum One / Sepolia, while soft sequencer confirmations occur in ~250ms, L1 batch poster reorgs or sequencer stalls can cause temporary chain reorganization in unfinalized blocks. Indexers that commit state at block head ($0$ confirmations) risk corrupting local tree mirrors if an unconfirmed transaction is reverted.
- **Mitigation in DEC-035C:**
  - The MMR indexer adopts the proven reorg-safe window architecture from [`DEC-018`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-018-on-chain-deposit-indexer-and-cryptographic-reveal-verification.md).
  - State checkpoints advance only up to:
    $$\text{safe\_block} = \text{latest\_block} - \text{SAFE\_CONFIRMATIONS}$$
  - If a deep reorg is detected, the indexer rolls back SQLite tables to the fork point and re-derives MMR peaks from the canonical chain.

---

## 3. Mathematical Foundations: Merkle Mountain Range (MMR)

An MMR is an append-only binary tree accumulator representing a list of $N$ leaves as a sequence of perfect binary trees (mountains) of decreasing heights ([Todd, 2012](https://github.com/opentimestamps/opentimestamps-server/blob/master/doc/merkle-mountain-range.md); [DEC-032](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md)).

### A. Mountain Structure from Binary Representation
Let $N$ be the total number of leaves. The binary decomposition of $N$:
$$N = \sum_{k=0}^{m} b_k \cdot 2^k, \quad b_k \in \{0, 1\}$$
determines the exact layout of the mountains:
- For every bit $b_k = 1$, there exists exactly one perfect binary tree of height $k$ containing $2^k$ leaves.
- The number of peaks is equal to the Hamming weight (number of 1-bits): $W(N) = \sum b_k \le \lfloor \log_2 N \rfloor + 1$.

*Example ($N = 11 = 8 + 2 + 1 = 2^3 + 2^1 + 2^0$):*
There are 3 peaks: Peak 0 (height 3, 8 leaves), Peak 1 (height 1, 2 leaves), Peak 2 (height 0, 1 leaf).

### B. Peak Bagging & Canonical Domain Separation
To produce a single 32-byte accumulator root $\mathcal{R}_N$, all $k$ peaks $[P_0, P_1, \dots, P_{k-1}]$ are folded from right to left using the audited Poseidon sponge with strict domain separation:
$$\text{Bagged Root } \mathcal{R}_N = \text{Poseidon}_{w5}([P_0, \dots, P_{k-1}, \text{zero}], \text{DOMAIN\_MMR\_BAG\_V1})$$

Where:
$$\text{DOMAIN\_MMR\_BAG\_V1} = \texttt{0x5a454c5452415f4d4d525f4241475f5631}$$

---

## 4. Invariants

- **INV-1 (Reorg-Safe Indexing):** The MMR indexer MUST only advance its confirmed peak checkpoint up to the safe finalized block window (`current_block - safe_confirmations`).
- **INV-2 (Deterministic Tree Mirror & Fail-Closed Integrity):** For every leaf index $N$, the local `MerkleMountainRange` mirror computed in `nimbus-node` MUST produce bit-exact root and peak equality with the on-chain event `bagged_root`. If any discrepancy occurs, the indexer MUST pause and alert immediately.
- **INV-3 (Zero-Knowledge Privacy Preservation):** The client sync endpoint MUST accept only integer positions (`leaf_index`), NEVER note commitments, hashes, or nullifiers.
- **INV-4 (Strict Boundary Validation):** The sync endpoint MUST reject any query where $\text{leaf\_index} \ge \text{leaf\_count}$ with `404 Not Found` (`LeafIndexOutOfBounds`), preventing out-of-bounds traversal attacks.
- **INV-5 (Atomic Checkpointing):** Database updates to `mmr_leaves` and `mmr_state` MUST execute within a single atomic SQLite transaction to prevent half-indexed states on unexpected node shutdown.

---

## 5. Technical Specification

### A. Database Persistence Schema (`nimbus-node`)
In `nimbus-node/src/db/`:
```sql
-- Individual indexed note commitment leaves
CREATE TABLE IF NOT EXISTS mmr_leaves (
    leaf_index INTEGER PRIMARY KEY,
    commitment TEXT NOT NULL,         -- 0x-prefixed 32-byte hex scalar
    tx_hash TEXT NOT NULL,
    block_number INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_mmr_leaves_block ON mmr_leaves(block_number);

-- Authoritative tracker of the current MMR tree state
CREATE TABLE IF NOT EXISTS mmr_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    leaf_count INTEGER NOT NULL,
    bagged_root TEXT NOT NULL,        -- 0x-prefixed 32-byte hex scalar
    peaks_json TEXT NOT NULL,         -- JSON array of active peak hex strings
    last_indexed_block INTEGER NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### B. Relayer MMR Indexer Worker (`mmr_indexer.rs`)

The indexer runs as a background task in `nimbus-node`:

```rust
pub struct MmrIndexer {
    provider: Arc<RootProvider<Http<Client>>>,
    contract_addr: Address,
    db: Arc<Database>,
    safe_confirmations: u64,
    poll_interval: Duration,
}

impl MmrIndexer {
    pub async fn run_loop(&self, mut shutdown: broadcast::Receiver<()>) {
        let mut interval = tokio::time::interval(self.poll_interval);
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.sync_step().await {
                        tracing::error!(target: "relayer::mmr_indexer", "MMR sync step failed: {:?}", e);
                    }
                }
                _ = shutdown.recv() => {
                    tracing::info!(target: "relayer::mmr_indexer", "MMR indexer shutting down cleanly");
                    break;
                }
            }
        }
    }

    async fn sync_step(&self) -> Result<(), IndexerError> {
        let latest_block = self.provider.get_block_number().await?;
        if latest_block < self.safe_confirmations {
            return Ok(());
        }
        let safe_block = latest_block - self.safe_confirmations;
        
        let last_indexed = self.db.get_mmr_last_indexed_block().await?;
        if last_indexed >= safe_block {
            return Ok(()); // Up to date
        }

        let from_block = last_indexed + 1;
        let to_block = std::cmp::min(from_block + MAX_BLOCK_RANGE, safe_block);

        // Filter NoteCommitmentAppended events
        let filter = Filter::new()
            .address(self.contract_addr)
            .event("NoteCommitmentAppended(uint256,bytes32,bytes32)")
            .from_block(from_block)
            .to_block(to_block);

        let logs = self.provider.get_logs(&filter).await?;
        if logs.is_empty() {
            self.db.update_mmr_indexed_block(to_block).await?;
            return Ok(());
        }

        // Reconstruct local MMR state
        let mut mmr = self.db.load_mmr_tree().await?;

        for log in logs {
            let event = NoteCommitmentAppended::decode_log(&log.inner, true)?;
            let leaf_index = event.leaf_index.to::<u64>();
            let commitment = event.commitment;
            let on_chain_root = event.bagged_root;

            // Invariant check: sequential indexing
            if leaf_index != mmr.leaf_count() {
                return Err(IndexerError::NonSequentialLeafIndex {
                    expected: mmr.leaf_count(),
                    got: leaf_index,
                });
            }

            // Append leaf to local MMR mirror
            let scalar_commitment = from_evm_scalar(&commitment.0)?;
            mmr.append_leaf(scalar_commitment)?;

            // FAIL-CLOSED INTEGRITY CHECK: Local mirror must equal on-chain event root
            let local_root = mmr.bagged_root()?;
            if local_root.to_be_bytes() != on_chain_root.0 {
                panic!(
                    "CRITICAL: MMR state desync! Local root: {:?}, On-chain root: {:?}",
                    local_root, on_chain_root
                );
            }

            self.db.insert_mmr_leaf(leaf_index, commitment, log.transaction_hash.unwrap(), log.block_number.unwrap()).await?;
        }

        // Atomically commit updated MMR state
        self.db.save_mmr_state(&mmr, to_block).await?;
        Ok(())
    }
}
```

### C. Relayer Client Sync REST API

`nimbus-node` exposes two endpoints under `/api/v1/mmr`:

#### 1. `GET /api/v1/mmr/proof/{leaf_index}`
Fetches the cryptographic inclusion proof for a specific leaf index.

**Parameters:**
- `leaf_index` (path, u64): Index of the note commitment in the MMR tree.

**Response (HTTP 200 OK):**
```json
{
  "leaf_index": 42,
  "leaf_count": 105,
  "commitment": "0x1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b",
  "mountain_height": 3,
  "mountain_siblings": [
    "0x2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c",
    "0x3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d",
    "0x4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e"
  ],
  "peak_bagging_siblings": [
    "0x5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f",
    "0x6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a"
  ],
  "bagged_root": "0x7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b",
  "block_number": 1234567
}
```

**Error Responses:**
- `400 Bad Request`: Invalid integer format.
- `404 Not Found`: `{"code": "LeafIndexOutOfBounds", "message": "leaf_index 105 exceeds current leaf_count 105"}`.

#### 2. `GET /api/v1/mmr/tip`
Returns current tree capacity and root. Used by wallets for fast synchronization.

**Response (HTTP 200 OK):**
```json
{
  "leaf_count": 105,
  "bagged_root": "0x7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b",
  "last_indexed_block": 1234567
}
```

### D. SDK Client Proof Fetching & Witness Generation (`nimbus-sdk`)

In `nimbus-sdk/src/wallet/note_wallet.rs`:
```rust
impl PrivateNoteWallet {
    pub async fn fetch_mmr_proof(
        &self,
        relayer_url: &str,
        leaf_index: u64,
    ) -> Result<MmrProofResponse, WalletError> {
        let url = format!("{}/api/v1/mmr/proof/{}", relayer_url, leaf_index);
        let resp = self.http_client.get(&url).send().await?;
        
        if resp.status() == StatusCode::NOT_FOUND {
            return Err(WalletError::NoteNotYetIndexed(leaf_index));
        }
        
        let proof_data: MmrProofResponse = resp.json().await?;
        Ok(proof_data)
    }

    pub fn build_circuit_witness(
        &self,
        note: &PrivateNote,
        proof_data: &MmrProofResponse,
    ) -> Result<PrivateNoteCircuitWitness, WalletError> {
        // 1. Convert siblings to Fr scalars
        let mountain_siblings: Vec<Fr> = proof_data
            .mountain_siblings
            .iter()
            .map(|hex| from_evm_scalar_hex(hex))
            .collect::<Result<Vec<_>, _>>()?;

        let peak_bagging_siblings: Vec<Fr> = proof_data
            .peak_bagging_siblings
            .iter()
            .map(|hex| from_evm_scalar_hex(hex))
            .collect::<Result<Vec<_>, _>>()?;

        // 2. Populate PrivateNoteCircuit
        Ok(PrivateNoteCircuitWitness {
            note_root: from_evm_scalar_hex(&proof_data.bagged_root)?,
            leaf_count: proof_data.leaf_count,
            mountain_height: proof_data.mountain_height,
            mountain_siblings,
            peak_bagging_siblings,
            // ... private note secrets ...
        })
    }
}
```

---

## 6. Threat Modeling & Security Analysis

```
┌───────────────────────────────────────┬────────────────────────────────────────────────────────┐
│ Threat Vector                         │ Architectural Mitigation / Mathematical Defense        │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 1. Out-of-Bounds Leaf Skipping        │ Strict validation: leaf_index < leaf_count.            │
│    (Hyperbridge Exploit Attack)       │ Endpoint returns 404; single-leaf proof check in ZK.   │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 2. Relayer Deanonymization Query      │ Privacy boundary enforced: Wallets query by integer    │
│    (Server clusters user transactions)│ position (leaf_index), NEVER by commitment/nullifier.  │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 3. L2 Sequencer Reorg Inconsistency   │ Safe block window: Indexer lags by SAFE_CONFIRMATIONS. │
│    (Temporary unfinalized fork)       │ DB updates are idempotent and rollback-safe.           │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 4. State Mirror Desynchronization     │ Fail-closed assertion: Local mmr.bagged_root() must    │
│    (Relayer computes wrong peaks)     │ match on-chain event. Discrepancy halts indexer.       │
└───────────────────────────────────────┴────────────────────────────────────────────────────────┘
```

---

## 7. Testing & Verification Matrix

Prior to Gate G testnet rehearsal, the following test matrix must pass:

### A. Indexer Mirror & Crash Consistency Tests (`nimbus-node`)
1. **Sequential Append Test:**
   - Ingest 500 mock events with random commitments.
   - Assert SQLite `leaf_count == 500`.
   - Assert `mmr_state.bagged_root == nimbus_core::MerkleMountainRange::bagged_root()`.
2. **Reorg Rollback Test:**
   - Simulate an L2 reorg where blocks $105 \dots 110$ are replaced.
   - Verify indexer rolls back `mmr_leaves` to block $104$ and successfully ingests the replacement branch.
3. **Fail-Closed Mismatch Panic Test:**
   - Inject an event with an intentionally altered `bagged_root`.
   - Assert indexer detects mismatch and halts execution without corrupting database state.

### B. Client Sync REST API Tests (`nimbus-node`)
1. **Valid Proof Query:**
   - Request `GET /api/v1/mmr/proof/0` through `GET /api/v1/mmr/proof/499`.
   - Verify all responses contain valid mountain siblings and peak bagging siblings.
2. **Out-of-Bounds Rejection:**
   - Query `leaf_index = 500` on a 500-leaf tree.
   - Assert HTTP `404 Not Found` with code `LeafIndexOutOfBounds`.

### C. End-to-End Multi-Wallet Integration (`nimbus-sdk` & `nimbus-contracts`)
1. **Cross-Wallet Spend Rehearsal:**
   - Wallet A deposits Note 1 (`leaf_index = 0`).
   - Wallet B deposits Note 2 (`leaf_index = 1`).
   - Wallet A fetches proof from relayer sync endpoint, constructs Groth16 spend proof, and successfully spends Note 1.
   - Verify contract verifies Groth16 proof against the on-chain MMR root without reverts.

---

## 8. References & Citations

1. **Todd, Peter.** (2012). *Merkle Mountain Ranges: An Append-Only Authenticated Data Structure.* OpenTimestamps Architecture Documentation. [https://github.com/opentimestamps/opentimestamps-server/blob/master/doc/merkle-mountain-range.md](https://github.com/opentimestamps/opentimestamps-server/blob/master/doc/merkle-mountain-range.md)
2. **Hyperbridge Security Team.** (2026). *Hyperbridge MMR Verification Vulnerability & Iterator Exhaustion Post-Mortem ($237K Exploit Analysis).* Hyperbridge Core Repository & Security Advisories. [https://github.com/hyperbridge/core](https://github.com/hyperbridge/core)
3. **Parity Technologies.** (2024). *Substrate Beefy MMR: Merkle Mountain Range Pallet & Security Reviews.* ParityTech Substrate Repository. [https://github.com/paritytech/merkle-mountain-range](https://github.com/paritytech/merkle-mountain-range)
4. **O'Grady, Sean; Meier, Raymond; & Policharla, Pratyush.** (2026). *Bonsai: Scalable Private Payments with Merkle Mountain Ranges and Compact Trees.* IACR Cryptology ePrint Archive, Report 2026/1987. [https://eprint.iacr.org/2026/1987](https://eprint.iacr.org/2026/1987)
5. **Döttling, Nico; & Faber, Luc.** (2025). *Merkle Mountain Ranges are Optimal: Asymptotic Lower Bounds for Dynamic Vector Commitments.* In: Advances in Cryptology – CRYPTO 2025. IACR Cryptology ePrint Archive, Report 2025/234. [https://eprint.iacr.org/2025/234](https://eprint.iacr.org/2025/234)
6. **Zcash Community.** (2020). *ZIP-314: Upgrade to Compact Block Light Wallet Protocol and Privacy Protection Boundaries.* Zcash Improvement Proposals. [https://zips.z.cash/](https://zips.z.cash/)
7. **Valverde, Gustavo.** (2025). *Zinder: Wallet Data Plane Architecture, Shielded Light Client Privacy Boundaries, and Tree State Anchors.* Zinder Repository Documentation. [https://github.com/gustavovalverde/zinder](https://github.com/gustavovalverde/zinder)
8. **Tanctl Community.** (2026). *RIME: Metadata-Resistant Zcash Unified Address Light Client with Oblivious Retrieval and Dummy Cadence.* [https://github.com/tanctl/rime](https://github.com/tanctl/rime)
9. **Offchain Labs.** (2024). *Arbitrum Nitro: Architecture, Finality, and Reorg Safety in Optimistic Rollups.* Arbitrum Documentation. [https://docs.arbitrum.io/build-decentralized-apps/how-to-estimate-gas](https://docs.arbitrum.io/build-decentralized-apps/how-to-estimate-gas)
10. **Grassesi, Lorenzo; Khovratovich, Dmitry; Rechberger, Christian; Roy, Arnab; & Schofnegger, Markus.** (2021). *Poseidon: A New Hash Function for Zero-Knowledge Proof Systems.* USENIX Security Symposium 2021. [https://eprint.iacr.org/2019/458](https://eprint.iacr.org/2019/458)
