# DEC-035C: Relayer MMR Indexer Worker & Client Sync Synchronization Architecture

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md))
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-node/src/mmr_indexer.rs` (New background daemon indexing on-chain commitments)
  - `nimbus-node/src/routes.rs` & `handlers/mmr.rs` (New `GET /api/v1/mmr/proof/{leaf_index}` endpoint)
  - `nimbus-node/src/db/schema.rs` (New SQLite table `mmr_leaves` and persistent peak snapshots)
  - `nimbus-sdk/src/wallet/note_wallet.rs` (Dynamic MMR inclusion proof sync client)
  - `nimbus-contracts/src/merkle.rs` (Event `NoteCommitmentAppended` emission verification)
- **Parent & Related DECs:**
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Master Zero-Tech-Debt Blueprint)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) (Merkle Mountain Range Commitment Accumulator)
  - [`DEC-018`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-018-on-chain-deposit-indexer-and-cryptographic-reveal-verification.md) (Deposit Indexer Architecture)

---

## 1. Problem Statement & Motivation

During the repository investigation across Claude, Gemini, and ChatGPT:
1. **The Missing Integration Block:**
   While the Stylus smart contract ([`nimbus-contracts/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/merkle.rs)) and the circuit ([`nimbus-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note.rs)) successfully implemented the Merkle Mountain Range (MMR) accumulator, the relayer node (`nimbus-node`) **completely lacked an MMR event indexer and synchronization API**.
2. **The Client Mock Fallback:**
   Because there was no relayer indexer or sync API, the client SDK was forced to use placeholder zeros or purely local single-node assumptions. Multi-wallet, multi-user spends in a live testnet environment were impossible: a wallet cannot prove membership of its commitment without knowing the current tree size ($N$) and sibling peaks from on-chain state.
3. **The Zero Tech Debt Mandate:**
   To satisfy Founder Invariant 2 ("Definitive Pre-Testnet Hardening"), Gate G (Arbitrum Sepolia) cannot be launched until the client-relayer-contract loop is 100% complete with a robust, persistent MMR synchronization pipeline.

---

## 2. Invariants

- **INV-1 (Reorg-Safe Indexing):** The MMR indexer MUST only advance its confirmed peak checkpoint up to the safe finalized block window (`current_block - safe_confirmations`).
- **INV-2 (Deterministic Tree Mirror):** The relayer's SQLite MMR mirror MUST produce bit-exact root and peak equality with the on-chain contract state at every leaf index $N$.
- **INV-3 (Zero-Knowledge Privacy Protection):** The client sync endpoint `GET /api/v1/mmr/proof/{leaf_index}` requests Merkle tree positions by integer index, NOT by note commitment or nullifier, preserving user transaction unobservability.

---

## 3. Specification

### A. Database Persistence Schema (`nimbus-node`)
In `nimbus-node/src/db/`:
```sql
CREATE TABLE IF NOT EXISTS mmr_leaves (
    leaf_index INTEGER PRIMARY KEY,
    commitment TEXT NOT NULL,         -- 0x-prefixed 32-byte hex scalar
    tx_hash TEXT NOT NULL,
    block_number INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS mmr_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    leaf_count INTEGER NOT NULL,
    bagged_root TEXT NOT NULL,
    last_indexed_block INTEGER NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### B. Relayer MMR Indexer Worker (`mmr_indexer.rs`)
1. **Event Listening:**
   Subscribe to and poll logs for the Stylus contract event:
   ```solidity
   event NoteCommitmentAppended(
       uint256 indexed leaf_index,
       bytes32 commitment,
       bytes32 bagged_root
   );
   ```
2. **Reconstruction & State Mirroring:**
   - On detecting new events within the finalized block window, batch-insert leaves into `mmr_leaves`.
   - Update an in-memory/persistent `MerkleMountainRange` instance from `nimbus-core`.
   - Verify that the local `mmr.bagged_root()` matches the event's `bagged_root`. If a discrepancy occurs, alert and pause (fail-closed integrity).
   - Atomically commit the updated state to `mmr_state`.

### C. Relayer Client Sync REST API
Expose endpoint for client wallets:
`GET /api/v1/mmr/proof/{leaf_index}`

**Response JSON Schema:**
```json
{
  "leaf_index": 42,
  "leaf_count": 105,
  "commitment": "0x1a2b...3c4d",
  "mountain_height": 3,
  "mountain_siblings": [
    "0x...",
    "0x...",
    "0x..."
  ],
  "peak_bagging_siblings": [
    "0x...",
    "0x..."
  ],
  "bagged_root": "0x5e6f...7a8b",
  "block_number": 1234567
}
```

### D. SDK Client Integration (`nimbus-sdk`)
In `nimbus-sdk/src/wallet/note_wallet.rs`:
1. When generating a Groth16 spend proof for an unspent note with index `leaf_index`:
   - Fetch the inclusion proof from `GET /api/v1/mmr/proof/{leaf_index}`.
   - Deserialize siblings into `ark_bls12_381::Fr`.
   - Populate `PrivateNoteCircuit` witness with verified mountain siblings, peak bagging siblings, and the on-chain `leaf_count`.
2. Ensure offline caching: if the tree has not changed, reuse cached siblings.

---

## 4. Test & Verification Plan

1. **Indexer Sync & Reorg Test:**
   - Ingest 100 mock `NoteCommitmentAppended` events.
   - Verify all 100 leaves persist in SQLite.
   - Assert SQLite state `bagged_root` matches `nimbus-core::MerkleMountainRange::bagged_root()`.
2. **End-to-End Client Sync Proof Verification:**
   - Append 10 random commitments.
   - Client requests proof for `leaf_index = 3`.
   - Client passes proof into `PrivateNoteCircuit`.
   - Run Groth16 prover $\rightarrow$ Groth16 verifier.
   - Assert proof passes verification with contract verifier.
3. **Invalid Index Boundary Test:**
   - Request `leaf_index >= leaf_count`.
   - Endpoint MUST return `404 Not Found` with `LeafIndexOutOfBounds`.
