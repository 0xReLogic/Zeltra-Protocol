# DEC-036C: Relayer Ingress Pipeline, Dual-Nullifier Guard, and Stylus Smart Contract 2-in-2-out Settlement with EIP-2537

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md))
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-node/src/handlers/spend.rs` (Relayer `/api/v1/spend-joinsplit` handler, dual-nullifier guards, and preflight Groth16 verification)
  - `nimbus-node/src/db.rs` (SQLite `spent_nullifiers` tracking and transactional persistence)
  - `nimbus-contracts/src/spend.rs` (Stylus on-chain `spend_joinsplit` entrypoint, multi-liability solvency invariant, and settlement)
  - `nimbus-contracts/src/merkle.rs` (On-chain MMR constant-arity dual commitment insertion `_mmr_insert`)
  - `nimbus-contracts/src/groth16_joinsplit_verifier.rs` (EIP-2537 precompile `0x0c` MSM and `0x0f` Pairing routines for 19 public inputs)
- **Parent & Related DECs:**
  - [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md) (Master JoinSplit Architecture Blueprint)
  - [`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md) (Universal 2-in-2-out JoinSplit R1CS Circuit & Soundness)
  - [`DEC-036B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md) (Client SDK Knapsack & In-Pool Consolidation)
  - [`DEC-017`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-017-relayer-receipt-finality-and-nonce-management.md) (Relayer Receipt Finality & Nonce Management)
  - [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) (Proof-Settlement Mismatch Defense)
  - [`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md) (Relayer ZK Note Spend Settlement Pipeline)
  - [`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md) (Three-Way Reconciliation & Solvency Observability)

---

## 1. Executive Summary & Problem Statement

Penyelesaian transaksi pengeluaran privat (*spend settlement*) dalam arsitektur Zeltra Protocol melibatkan interaksi dua lapis eksekusi:
1. **Lapisan Relayer Node (`nimbus-node`):** Bertindak sebagai gateway publik yang menerima payload transaksi dari klien, memverifikasi keabsahan kriptografis dan semantik sebelum memancarkan transaksi ke blockchain, serta melindungi modal relayer dari serangan pengurasan gas (*gas-griefing attacks*).
2. **Lapisan Smart Contract Stylus WASM (`nimbus-contracts`):** Menjadi penegak kebenaran akhir (*ultimate arbiter of truth*) di blockchain Arbitrum, memverifikasi bukti Groth16 melalui host precompiles EIP-2537 (`0x0c` MSM dan `0x0f` Pairing), membakar nullifier, menyisipkan daun komitmen baru ke akumulator MMR, mentransfer dana ke merchant, dan memvalidasi invarian solvensi multi-liability.

Dalam skema JoinSplit 2-in-2-out ([`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md)), setiap transaksi mengonsumsi **dua buah nullifier sekaligus** dan menghasilkan **dua buah komitmen daun baru** ke pohon MMR. 

**DEC-036C** menetapkan arsitektur lengkap pipeline relayer ingress dan smart contract on-chain untuk mengeksekusi penyelesaian JoinSplit 2-in-2-out secara fail-closed, atomik, dan kebal terhadap serangan double-spending serta eksploitasi gas.

---

## 2. Vulnerability Archaeology & Threat Model

Ekosistem relayer dan verifier smart contract pada protokol zero-knowledge sering kali menjadi sasaran empuk serangan finansial, pengurasan modal, dan eksploitasi mempool:

```
                            SETTLEMENT LAYER THREAT VECTORS
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ [Case A: EIP-2537 Gas Burn]       [Case B: Singularity Aliasing] [Case C: Frontrunning]│
│ EVM Threat Models 2026: Precompile Zellic 2024: nf1 == nf2       OpenZeppelin / Railgun│
│ errors burn ALL forwarded gas     duplicated balances in single  Relayer fee hijacking │
│ -> Relayer reserves drained to 0  transaction without revert     in public mempool     │
├───────────────────────────────────┼──────────────────────────────┼─────────────────────┤
│ [Case D: Non-Canonical Reverts]   [Case E: Stale Return Buffers] [Case F: Reentrancy]  │
│ Scalars >= r trigger EVM abort;   Contracts failing to verify    Malicious recipient   │
│ relayer pays gas with 0 fee refund returndatasize() read garbage re-enters before      │
│ without ingress range assertions  data from memory buffers       multi-liability check │
└───────────────────────────────────┴──────────────────────────────┴─────────────────────┘
```

### A. EIP-2537 BLS12-381 Precompile: Burn-All-Gas-On-Error Semantics (EVM Threat Models, 2026)
- **Konteks Kerentanan:** Dalam analisis model ancaman precompile EVM ([Krash.dev EVM Threat Models: `0x0F-BLS12_PAIRING_CHECK`, Maret 2026](https://krash.dev/evm-tm/Precompile-Threat-Models/0x0F-BLS12_PAIRING_CHECK); [EIP-2537 Specification](https://eips.ethereum.org/EIPS/eip-2537)), teridentifikasi aturan eksekusi kritis: **Gas burning on error**.
- **Root Cause:** Tidak seperti instruksi EVM biasa yang mengembalikan sisa gas (*unused gas refund*) ketika terjadi revert, panggilan ke precompiles EIP-2537 (`0x0c` MSM dan `0x0f` Pairing) yang mengalami error (titik di luar kurva, bukan anggota subgrup yang sah, skalar $\ge r$, atau panjang input yang tidak habis dibagi 160/384 bytes) akan **MEMBAKAR SELURUH GAS YANG DITERUSKAN DALAM CALL/STATICCALL TANPA SISA** dan tidak menghasilkan return data apa pun.
  Jika sebuah node relayer menerima payload dari pengguna tanpa memverifikasi bukti secara lokal dan langsung memancarkan transaksi on-chain dengan alokasi gas limit 800.000 gas, transaksi yang invalid akan membakar seluruh 800.000 gas tersebut. Penyerang dapat membanjiri relayer dengan ribuan bukti palsu gratis, menguras saldo ETH relayer hingga nol dalam hitungan menit (*economic denial of service / relayer bankruptcy attack*).
- **Relevansi & Mitigasi Zeltra (DEC-036C):**
  Relayer Zeltra **WAJIB** mengeksekusi **Local CPU Groth16 Preflight Verification** menggunakan `ark-groth16` pada lapisan ingress HTTP sebelum transaksi dipancarkan ke antrean RPC. Selain itu, relayer memvalidasi bahwa seluruh 19 public inputs berukuran skalar kanonikal ($< r$). Bukti yang invalid ditolak seketika dengan HTTP `400 Bad Request` tanpa melibatkan transaksi on-chain sama sekali.

---

### B. Singularity Darkpool: Multi-Input Nullifier Aliasing Exploitation (Zellic Audit, 2024)
- **Konteks Kerentanan:** Audit Zellic terhadap Singularity Darkpool ([Zellic Audit Reports, 2024](https://reports.zellic.io/publications/singularity/findings/critical-darkpoolassetmanager-double-spend-possible-within-actions-taking-two-notes-as-input/)) menemukan kerentanan **CRITICAL** pada fungsi yang memproses multi-input notes.
- **Root Cause:** Kontrak memeriksa bahwa `nullifier_1` dan `nullifier_2` belum terdaftar di mapping `spent`, tetapi tidak menegakkan bahwa `nullifier_1 != nullifier_2`. Karena mapping penandaan bersifat idempoten (`nullifiersUsed[nullifier] = true`), penyerang memasukkan note yang sama bernilai $A$ pada kedua slot input. Kedua nullifier identik disetujui, dan penyerang memperoleh output senilai $2A$, menguras seluruh kolam likuiditas.
- **Relevansi & Mitigasi Zeltra (DEC-036C):**
  Smart contract Stylus `spend_joinsplit` secara eksplisit menegakkan pengecekan non-aliasing sebelum mutasi storage:
  ```rust
  require(nullifier_1 != nullifier_2, "DuplicateNullifierInSameTx");
  require(!self.note_nullifiers.get(nullifier_1), "Nullifier1AlreadySpent");
  require(!self.note_nullifiers.get(nullifier_2), "Nullifier2AlreadySpent");
  ```
  Relayer juga menegakkan pengecekan `assert!(req.nullifier_1 != req.nullifier_2)` pada filter ingress pertama.

---

### C. Relayer Frontrunning & Fee Hijacking in Shielded Mempools (Railgun / OpenZeppelin, 2022–2024)
- **Konteks Kerentanan:** Dalam protokol pembayaran privat dengan pihak ketiga relayer (analisis OpenZeppelin pada Railgun dan Tornado Cash), penyerang mengamati mempool publik L1/L2 untuk mencari transaksi spend yang dipancarkan oleh relayer sah.
- **Root Cause:** Jika kontrak mengizinkan sembarang pemanggil (`msg.sender`) menerima biaya eksekusi relayer tanpa mengikat identitas pemanggil ke bukti ZK atau kuotasi bertanda tangan, penyerang dapat menyalin seluruh payload proof dan memancarkannya dengan gas fee lebih tinggi (*frontrunning/MEV sandwich*), mencuri biaya `execution_fee` yang seharusnya menjadi hak relayer penyedia kuotasi.
- **Relevansi & Mitigasi Zeltra (DEC-036C):**
  Kuotasi eksekusi `ExecutionQuote` ditandatangani oleh pengguna menggunakan standar EIP-712 ([`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md)), di mana digest kuotasi dipecah menjadi Two-Limb (`quote_hash_hi`, `quote_hash_lo`) yang terikat ke dalam sirkuit R1CS. Kontrak memvalidasi kesesuaian parameter quote dan mentransfer `execution_fee` secara ketat kepada `msg.sender` yang diotorisasi, mengeliminasi risiko pembajakan kuotasi di mempool.

---

### D. Reentrancy & Cross-Function Solvency Invariant Violations
- **Konteks Kerentanan:** Transfer token ERC-20 eksternal (USDC) ke alamat `recipient` membuka potensi panggilan balik (*reentrancy*) jika penerima adalah smart contract dengan hook token (ERC-777/ERC-1363 atau fallback router).
- **Root Cause:** Jika transfer dana dilakukan sebelum status nullifier diperbarui atau sebelum liabilitas dicatat, penyerang dapat memanggil kembali fungsi spend di tengah-tengah eksekusi untuk menggandakan pengeluaran.
- **Relevansi & Mitigasi Zeltra (DEC-036C):**
  Kontrak Stylus secara ketat menerapkan pola **Checks-Effects-Interactions (CEI)**:
  1. *Checks:* Verifikasi expiry, nullifier non-aliasing, skalar kanonikal, dan bukti Groth16.
  2. *Effects:* Tandai kedua nullifier sebagai spent di storage dan lakukan 2x `_mmr_insert` ke akumulator pohon.
  3. *Interactions:* Transfer USDC ke `recipient` dan transfer `execution_fee` ke relayer.
  4. *Solvency Guard:* Di baris paling akhir, jalankan `_assert_solvency_invariant()` yang memverifikasi bahwa saldo USDC kontrak $\ge$ total liabilitas. Revert seketika jika terjadi insolvensi.

---

## 3. System Invariants

- **INV-RELAY-1 (Fail-Closed Canonical Public Scalars):** Seluruh 19 public inputs wajib divalidasi $< r$ via `from_evm_scalar`. Jika terdapat satu elemen $\ge r$, relayer menolak payload sebelum deserialisasi ke tipe medan sirkuit.
- **INV-RELAY-2 (Atomic Nullifier Reservation):** Kedua nullifier wajib dicatat ke dalam database SQLite relayer dalam satu transaksi SQL terisolasi sebelum transaksi dipancarkan ke RPC.
- **INV-SC-1 (Strict Constant-Arity MMR Appends - Direction B):** Smart contract wajib mengeksekusi tepat 2 kali pemanggilan `_mmr_insert` pada setiap pemanggilan `spend_joinsplit`:
  $$\text{\_mmr\_insert}(\text{output\_commitment\_1}) \quad \text{dan} \quad \text{\_mmr\_insert}(\text{output\_commitment\_2})$$
  menjamin pertumbuhan pohon daun yang seragam tanpa kebocoran metadata ariti (meniadakan ZIP 315 leakage).
- **INV-SC-2 (Non-Aliased Nullifier Uniqueness):**
  $$\text{nullifier\_1} \ne \text{nullifier\_2} \quad \land \quad \text{spent}[\text{nullifier\_1}] == \text{false} \quad \land \quad \text{spent}[\text{nullifier\_2}] == \text{false}$$
- **INV-SC-3 (Mathematical Solvency Conservation):**
  $$\text{Contract USDC Balance} \ge \text{Total Liabilities} = (\text{user\_liabilities} + \text{refundable\_liabilities} + \text{accrued\_fees})$$
  diverifikasi sebelum eksekusi transaksi selesai.

---

## 4. Relayer Ingress Pipeline Specification

```
                         RELAYER INGRESS PIPELINE
               POST /api/v1/spend-joinsplit (JoinSplitPayload)
                                     │
                                     ▼
                ┌────────────────────────────────────────┐
                │ 1. Canonical Scalar Check (< r)        │
                │    Validate all 19 public inputs       │
                └────────────────────┬───────────────────┘
                                     │ Pass
                                     ▼
                ┌────────────────────────────────────────┐
                │ 2. Nullifier Non-Aliasing & Local Lock │
                │    assert!(nf_1 != nf_2)               │
                │    Check & lock SQLite spent_nullifers │
                └────────────────────┬───────────────────┘
                                     │ Pass
                                     ▼
                ┌────────────────────────────────────────┐
                │ 3. On-Chain Simulation Verification    │
                │    eth_call: is_nullifier_spent(nf1/2) │
                └────────────────────┬───────────────────┘
                                     │ Pass
                                     ▼
                ┌────────────────────────────────────────┐
                │ 4. EIP-712 Quote & Fee Semantic Binding│
                │    Verify quote signature & min fee    │
                └────────────────────┬───────────────────┘
                                     │ Pass
                                     ▼
                ┌────────────────────────────────────────┐
                │ 5. Local CPU Groth16 Preflight Verifier│
                │    ark-groth16 verify_proof() on CPU   │
                │    (PREVENTS PRECOMPILE GAS-GRIEFING!) │
                └────────────────────┬───────────────────┘
                                     │ Valid
                                     ▼
                ┌────────────────────────────────────────┐
                │ 6. Broadcast to Arbitrum Sepolia RPC   │
                │    Record tx_hash, await receipt       │
                └────────────────────────────────────────┘
```

### Handler Ingress Rust (`nimbus-node/src/handlers/spend.rs`):
```rust
pub async fn handle_spend_joinsplit(
    State(state): State<Arc<RelayerState>>,
    Json(payload): Json<JoinSplitSpendRequest>,
) -> Result<Json<SpendResponse>, RelayerError> {
    // Step 1: Validasi ketat bahwa kedua nullifier tidak identik (Anti-Aliasing)
    if payload.input_nullifier_1 == payload.input_nullifier_2 {
        return Err(RelayerError::BadRequest("DuplicateNullifierInSameTx".into()));
    }

    // Step 2: Validasi kanonikalitas 19 public inputs (< r)
    let public_inputs = payload.parse_and_validate_public_inputs()?;

    // Step 3: Pengecekan dual nullifier di SQLite lokal
    state.db.lock_nullifiers_atomic(&payload.input_nullifier_1, &payload.input_nullifier_2).await?;

    // Step 4: Simulasi on-chain view (is_nullifier_spent)
    let is_spent_1 = state.contract_client.is_nullifier_spent(payload.input_nullifier_1).await?;
    let is_spent_2 = state.contract_client.is_nullifier_spent(payload.input_nullifier_2).await?;
    if is_spent_1 || is_spent_2 {
        state.db.release_nullifiers(&payload.input_nullifier_1, &payload.input_nullifier_2).await?;
        return Err(RelayerError::Conflict("NullifierAlreadySpentOnChain".into()));
    }

    // Step 5: Validasi tanda tangan EIP-712 quote & expiry
    state.validate_execution_quote(&payload.quote, &payload.quote_signature)?;

    // Step 6: Verifikasi preflight Groth16 pada CPU lokal via ark-groth16
    // Melindungi relayer dari pembakaran gas precompile EIP-2537!
    let proof = payload.decode_groth16_proof()?;
    let is_valid = state.groth16_verifier.verify(&public_inputs, &proof)?;
    if !is_valid {
        state.db.release_nullifiers(&payload.input_nullifier_1, &payload.input_nullifier_2).await?;
        return Err(RelayerError::BadRequest("InvalidGroth16ProofPreflight".into()));
    }

    // Step 7: Broadcast transaksi ke Arbitrum Sepolia RPC
    let tx_hash = state.dispatcher.broadcast_joinsplit_spend(payload).await?;
    Ok(Json(SpendResponse { tx_hash }))
}
```

---

## 5. Stylus Smart Contract Specification (`spend_joinsplit`)

### A. ABI Entrypoint Definitive (`nimbus-contracts/src/spend.rs`):
```rust
pub fn spend_joinsplit(
    &mut self,
    proof: Bytes,
    note_root: FixedBytes<32>,
    leaf_count: u64,
    nullifier_1: FixedBytes<32>,
    nullifier_2: FixedBytes<32>,
    output_cm_1: FixedBytes<32>,
    output_cm_2: FixedBytes<32>,
    recipient: Address,
    merchant_amount: u64,
    protocol_fee: u64,
    execution_fee: u64,
    quote_hash: FixedBytes<32>,
    expiry: u64,
    flags_packed: u8,
    is_rollover: u8,
) -> Result<(), Vec<u8>>
```

### B. Alur Eksekusi Smart Contract:
1. **Verifikasi Batas Waktu (Expiry):**
   ```rust
   let current_time = block::timestamp();
   if current_time > expiry {
       return Err("QuoteExpired".into());
   }
   ```
2. **Dual-Nullifier Non-Aliasing & Spent Guard:**
   ```rust
   if nullifier_1 == nullifier_2 {
       return Err("DuplicateNullifierInSameTx".into());
   }
   if self.note_nullifiers.get(nullifier_1) {
       return Err("Nullifier1AlreadySpent".into());
   }
   if self.note_nullifiers.get(nullifier_2) {
       return Err("Nullifier2AlreadySpent".into());
   }
   ```
3. **Konversi Quote Hash ke Dua Limb (DEC-035A):**
   ```rust
   let quote_hash_bytes = quote_hash.as_slice();
   let quote_hash_hi = &quote_hash_bytes[0..16]; // Big-endian high 128-bit
   let quote_hash_lo = &quote_hash_bytes[16..32]; // Big-endian low 128-bit
   ```
4. **Verifikasi Bukti Groth16 via Host Precompiles EIP-2537:**
   - Siapkan calldata linear combination untuk MSM `0x0c` (20 titik $IC$).
   - Siapkan calldata 4 pasangan titik $(P_i, Q_i)$ untuk Pairing Check `0x0f`:
     $$e(-A, B) \cdot e(\alpha, \beta) \cdot e(L, \gamma) \cdot e(C, \delta) == 1$$
   - Revert jika verifikasi menghasilkan nilai selain `0x01`.
5. **Pencatatan Status Nullifier Terbakar:**
   ```rust
   self.note_nullifiers.setter(nullifier_1).set(true);
   self.note_nullifiers.setter(nullifier_2).set(true);
   ```
6. **Direction B Constant-Arity MMR Appends:**
   ```rust
   // Kontrak WAJIB selalu menyisipkan tepat 2 daun komitmen ke MMR on-chain
   self._mmr_insert(output_cm_1)?;
   self._mmr_insert(output_cm_2)?;
   ```
7. **Penyelesaian Finansial & Transfer Dana (CEI Pattern):**
   - Transfer USDC ke `recipient` sebesar `merchant_amount`.
   - Akumulasi liabilitas fee protokol dan fee eksekusi:
     $$\text{accrued\_protocol\_fee\_liability} += \text{protocol\_fee}$$
     $$\text{accrued\_execution\_fee\_liability} += \text{execution\_fee}$$
   - Transfer `execution_fee` ke pemancar transaksi (`msg::sender()`).
8. **Validasi Invarian Solvensi Mutlak:**
   ```rust
   self._assert_solvency_invariant()?;
   ```

---

## 6. EIP-2537 Host Precompile Verifier Routine

Verifikasi Groth16 BLS12-381 pada Stylus memanfaatkan dua precompiles host EIP-2537:

```
                            GROTH16 VERIFICATION FLOW
┌──────────────────────────────────────────────────────────────────────────────────┐
│ 1. Public Inputs to G1 Linear Combination (MSM Precompile 0x0c):                 │
│    L = IC_0 + sum_{i=0}^{18} (public_inputs[i] * IC_{i+1})                       │
│    Total points: 20 G1 elements, 19 scalar multiplications                       │
│    Input buffer: 20 * 160 = 3,200 bytes                                          │
└────────────────────────────────────────┬─────────────────────────────────────────┘
                                         │ L computed (128 bytes G1)
                                         ▼
┌──────────────────────────────────────────────────────────────────────────────────┐
│ 2. Pairing Check (Precompile 0x0f):                                              │
│    e(-A, B) * e(alpha, beta) * e(L, gamma) * e(C, delta) == 1                   │
│    Input buffer: 4 pairs * 384 bytes = 1,536 bytes                               │
│    Output: 32 bytes (must be 0x00..01)                                           │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### Gas Overhead Analysis:
- Precompile `0x0c` (G1 MSM untuk 20 titik $IC$): $\approx 18.000$ gas L2.
- Precompile `0x0f` (BLS12 Pairing Check 4-pair): $37.700 + (32.600 \times 4) = 168.100$ gas L2.
- Biaya insert 1 daun ekstra dummy ke MMR (Direction B): $\approx 2.100$ gas L2 ($< \$0.0001$).
- **Total Overhead Eksekusi Kriptografi:** $\approx 188.200$ gas L2 (sangat hemat untuk L2 Arbitrum Sepolia / Nitro).

---

## 7. Target Network Precompile Status & Testnet Strategy

### A. Arbitrum Sepolia (`chain_id: 421614`):
- **Status:** **VERIFIED & ACTIVE (Gate G Target)**.
- Menjalankan ArbOS 40+ yang telah mengaktifkan precompiles EIP-2537 (`0x0b` s/d `0x12`).
- Baseline 13/13 pairing check negative tests telah lolos pada contract deployment sebelumnya.
- Rilis `spend_joinsplit` akan diuji penuh secara E2E di testnet ini.

### B. Arbitrum One (`chain_id: 42161`):
- **Status:** **BLOCKED / GATED**.
- Aktivasi precompile pada mainnet bergantung pada voting governance ArbitrumDAO dan rilis ArbOS 40/ArbOS 51 validator nodes.
- Deployment produksi ke Arbitrum One di-gate dengan static probe contract call sebelum transaksi diizinkan.

---

## 8. Acceptance Criteria & Negative Test Matrix

Implementasi `spend_joinsplit` pada `nimbus-node` dan `nimbus-contracts` wajib lulus pengujian ketat dengan rasio tes negatif $\ge 2\times$ dari tes positif:

1. **Positive Spend 1-in JoinSplit:**
   - 1 note real + 1 dummy input $\rightarrow$ 1 change real + 1 dummy change output.
   - Payout sukses, kedua nullifier terbakar, 2 daun masuk ke MMR, invarian solvensi valid.
2. **Positive Spend 2-in JoinSplit:**
   - 2 note real $\rightarrow$ 2 change notes real. Payout sukses, saldo terpecah terkonsolidasi.
3. **Negative Test 1 (Nullifier Aliasing Defense):**
   - Mengirimkan `nullifier_1 == nullifier_2`: revert dengan `"DuplicateNullifierInSameTx"`.
4. **Negative Test 2 (Double-Spend Nullifier 1):**
   - Membelanjakan `nullifier_1` yang telah tercatat spent: revert dengan `"Nullifier1AlreadySpent"`.
5. **Negative Test 3 (Double-Spend Nullifier 2):**
   - Membelanjakan `nullifier_2` yang telah tercatat spent: revert dengan `"Nullifier2AlreadySpent"`.
6. **Negative Test 4 (Invalid Groth16 Proof Preflight Defense):**
   - Mengubah 1 byte pada proof: ditolak oleh relayer preflight (HTTP 400), memvalidasi bahwa relayer tidak kehilangan gas on-chain.
7. **Negative Test 5 (Precompile Input Length Mismatch Defense):**
   - Mengirimkan payload pairing yang tidak habis dibagi 384 bytes: relayer menolak di ingress layer.
8. **Negative Test 6 (Expired Quote Rejection):**
   - Memanggil dengan `block.timestamp > expiry`: revert dengan `"QuoteExpired"`.
9. **Negative Test 7 (Insolvent Payout Attempt):**
   - Mengirimkan nilai $v_{\text{merchant}}$ melebihi saldo liabilitas: revert pada `_assert_solvency_invariant`.

---

## 9. References & Citations

1. **Ethereum Foundation.** (2020). *EIP-2537: Precompiled contracts for BLS12-381 curve operations.* Ethereum Improvement Proposals. [https://eips.ethereum.org/EIPS/eip-2537](https://eips.ethereum.org/EIPS/eip-2537)
2. **Krash.dev EVM Threat Models.** (2026). *Threat Model: 0x0F — BLS12_PAIRING_CHECK (Burn-All-Gas-on-Error Semantics & Variable-Length Input Hazards).* EVM Precompile Threat Analysis. [https://krash.dev/evm-tm/Precompile-Threat-Models/0x0F-BLS12_PAIRING_CHECK](https://krash.dev/evm-tm/Precompile-Threat-Models/0x0F-BLS12_PAIRING_CHECK)
3. **Zellic Security Team.** (2024). *Singularity Darkpool Audit Report: Critical — Double spend possible within actions taking two notes as input (`joinSplit`, `join`, `swap`).* Zellic Publications. [https://reports.zellic.io/publications/singularity/findings/critical-darkpoolassetmanager-double-spend-possible-within-actions-taking-two-notes-as-input/](https://reports.zellic.io/publications/singularity/findings/critical-darkpoolassetmanager-double-spend-possible-within-actions-taking-two-notes-as-input/)
4. **ZK-Security.** (2025). *Audit of Aleph Zero Shielder: Public Nullifiers, Frontrunning Hazards, and State Synchronization in Shielded Pools.* ZK-Security Public Reports. [https://reports.zksecurity.xyz/reports/aleph-zero-shielder/](https://reports.zksecurity.xyz/reports/aleph-zero-shielder/)
5. **Arbitrum Offchain Labs.** (2024). *ArbOS 40 Release: Activation of Cancun / Dencun & EIP-2537 BLS12-381 Precompiles on Nitro and Stylus.* Arbitrum Developer Documentation. [https://docs.arbitrum.io/run-a-node/arbos-releases/arbos40](https://docs.arbitrum.io/run-a-node/arbos-releases/arbos40)
6. **OpenZeppelin.** (2023). *Railgun Privacy System Security Assessment: Relayer Ingress, Quote Expirations, and Nullifier Registries.* OpenZeppelin Security Audits. [https://www.openzeppelin.com/security-audits](https://www.openzeppelin.com/security-audits)
7. **ABDK Consulting.** (2022). *Tornado Cash Nova Security Review: Multi-Input UTXO Nullifier Management and Relayer Gas Settlement.* ABDK Security Research. [https://www.abdk.consulting](https://www.abdk.consulting)
8. **Arkworks Community.** (2024). *ark-groth16: Efficient Groth16 Prover and Verifier in Rust.* Arkworks Ecosystem. [https://github.com/arkworks-rs/groth16](https://github.com/arkworks-rs/groth16)
9. **Ethereum Foundation.** (2018). *EIP-712: Typed structured data hashing and signing.* Ethereum Improvement Proposals. [https://eips.ethereum.org/EIPS/eip-712](https://eips.ethereum.org/EIPS/eip-712)
