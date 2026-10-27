# DEC-056: Cross-Chain CCIP Spend Sanitation, Double-Spend Bypass Elimination & Unified Multi-Chain Lifecycle Monitor

- **Status:** APPROVED ARCHITECTURAL SPECIFICATION & RATIFIED
- **Tipe DEC:** SECURITY FIX / AUDIT & FITUR BARU & INFRASTRUCTURE HARDENING
- **Author:** Zeltra Core Cryptography & Systems Team
- **Date:** 2026-10-27
- **Target Crates & Components:**
  - `zeltra-contracts/src/spend.rs` (Sanitasi fungsi legacy inversion `_ccip_receive`, pemurnian modul spend ZK-UTXO)
  - `zeltra-contracts/src/ccip_deposit.rs` (Kanonikal inbound receiver hook DEC-055, integrasi event `CCIPDepositExecuted`)
  - `zeltra-node/src/batch_processor.rs` (Eliminasi celah bypass spend cross-chain, penegakan pembakaran nullifier di Arbitrum sebelum dispatch)
  - `zeltra-node/src/evm_client.rs` (Sanitasi token outflow CCIP v2: `tokenAmounts` nyata, `extraArgs` V2/V3 kanonikal, validasi `isChainSupported`)
  - `zeltra-node/src/ccip_monitor.rs` (Unified lifecycle monitor daemon untuk Inbound Deposit & Outbound Spend)
  - `zeltra-node/src/database/ccip.rs` (Penyempurnaan relasional status pelacakan pesan CCIP, deposit indexer, dan refund guard)
  - `zeltra-node/src/handlers/` & `zeltra-node/src/router.rs` (Endpoint publik monitoring `/api/v1/ccip/deposit/:id`, `/api/v1/ccip/spend/:id`, `/api/v1/ccip/status/:id`)
- **Parent & Related DECs:**
  - [`DEC-055: Cross-Chain CCIP Programmable Inbound Deposit & MMR Minting`](DEC-055-cross-chain-ccip-programmable-inbound-deposit-and-mmr-minting.md) (Inflow rail receiver interface)
  - [`DEC-038B: Direct MMR Note Deposit & Zero-Fee Inflow Architecture`](DEC-038B-direct-mmr-note-deposit-and-zero-fee-inflow-architecture.md) (0 bps fee deposit invariant)
  - [`DEC-043B: Strict 3-Mode JoinSplit`](DEC-043B-strict-three-mode-joinsplit-and-malleability-elimination.md) (ZK-UTXO spend & nullifier burning rules)
  - [`DEC-035C: Merkle Mountain Range On-Chain Indexer Daemon`](DEC-035C-mmr-onchain-indexer-daemon-and-reorg-safe-reconstruction.md) (Event indexing & MMR state)
  - [`DEC-054: Zero-Custody Regulatory Safe Harbor & Direct Self-Broadcast Architecture`](DEC-054-zero-custody-safe-harbor-and-direct-self-broadcast.md) (Eliminasi relayer escrow custody)
- **Security Audit Targets:** `AUD-CCIP-01` (Double-Spend Bypass), `AUD-CCIP-02` (Tokenless Outflow), `AUD-CCIP-03` (Selector & ERC-165 Mismatch), `AUD-CCIP-04` (False Relayer Finality)
- **Scientific Foundation & Primary Citations:**
  - Chainlink CCIP v1.6 / v2.0 Architecture & Specification (`@chainlink/contracts-ccip` v2.0.0, OpenAPI v2.13.0)
  - ERC-165: Standard Interface Detection (`supportsInterface`)
  - EIP-712: Typed Structured Data Hashing and Signing

---

## 1. Executive Summary & Problem Formulation

### A. Konteks & Problem Statement

Pada perancangan awal integrasi cross-chain (era pra-DEC-055), Chainlink CCIP diimplementasikan secara terburu-buru dengan menaruh logika cross-chain di dalam modul spend (`zeltra-contracts/src/spend.rs`) dan prosesor batch relayer (`zeltra-node/src/batch_processor.rs`). Berdasarkan audit forensik mendalam yang diidentifikasi pada DEC-055, implementasi tersebut mengandung **7 Celah Fatal** yang memicu kerentanan kritis, inkonsistensi arsitektur, dan potensi *infinite double-spend*:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                   FORENSIK 7 CELAH FATAL CCIP SPEND & PENYELESAIANNYA                  │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 1. INVERSI KONSEPTUAL & SALAH KAMAR:                                                   │
│    Fungsi ccipReceive ditaruh di spend.rs seolah-olah itu fungsi penarikan dana keluar.│
│    Solusi: ccipReceive adalah hook INBOUND; seluruh penerimaan CCIP dipusatkan di      │
│    ccip_deposit.rs (DEC-055). Dead code _ccip_receive di spend.rs dihapus tuntas.      │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 2. ABI SELECTOR MISMATCH (0x7b629c41 vs 0x3865668b):                                   │
│    Solusi: Ratifikasi Any2EVMMessage kanonikal dengan selector 0x3865668b (DEC-055).   │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 3. ERC-165 INTERFACE DETECTION MISSING:                                                │
│    Solusi: supports_interface(0x8a92dced) telah diimplementasikan di Stylus (DEC-055). │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 4. TOKENLESS OUTFLOW (HAMPA TOKEN):                                                    │
│    Relayer broadcast_ccip_transaction mengirim tokenAmounts: vec![]. Merchant tidak    │
│    menerima dana apapun di jaringan tujuan!                                            │
│    Solusi: Konfigurasi EVMTokenAmount nyata, approval token, dan CCIP v2 ExtraArgs.    │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 5. CRITICAL DOUBLE-SPEND BYPASS DI RELAYER (FATAL):                                    │
│    batch_processor.rs baris 152-166 mengeksekusi broadcast CCIP tanpa verifikasi proof│
│    dan tanpa membakar nullifier di Stylus Arbitrum! Pengguna bisa double-spend note.   │
│    Solusi: Eksekusi ZK Proof & pembakaran nullifier di Stylus WAJIB berhasil sebelum   │
│    relayer boleh memproses transfer lintas rantai apapun.                              │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 6. RELAYER FALSE FINALITY:                                                             │
│    Relayer menandai spend 'confirmed' hanya karena transaksi terkirim, tanpa menunggu  │
│    status eksekusi di rantai tujuan.                                                   │
│    Solusi: Status dipisah menjadi local Arbitrum status dan cross-chain delivery state.│
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 7. KETIADAAN UNIFIED MONITORING (DEPOSIT & SPEND):                                     │
│    ccip_monitor.rs hanya memantau single OffRamp event via RPC konvensional.           │
│    Solusi: Bangun Unified CCIP Lifecycle Monitor memanfaatkan Chainlink CCIP v2 API    │
│    (https://api.ccip.chain.link/v2) + On-Chain State fallback untuk Deposit dan Spend. │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### B. Threat Model & Financial Failure Scenario

1. **Infinite Double-Spend Exploit via Bypass:**
   Jika cabang `request.cross_chain` tidak mewajibkan eksekusi transaksi smart contract di Arbitrum, penyerang dapat membuat transaksi privat palsu berulang kali, menguras likuiditas relayer tanpa pernah mengurangi liabilitas note mereka di Arbitrum.
2. **Ghost Settlement (Merchant Non-Payment):**
   Pesan data tanpa token (`tokenAmounts: []`) yang tiba di rantai tujuan tidak dapat membayar merchant, memicu kegagalan komersial protokol.
3. **Desinkronisasi Status Monitoring:**
   Tanpa pemantauan siklus hidup CCIP resmi, pengguna yang menyetor via deposit CCIP tidak mengetahui apakah komitmen note mereka sudah tertanam di MMR Arbitrum atau masih dalam perjalanan di oracle DON.

---

## 2. Invariants & Batasan Non-Negotiable

1. **Arbitrum Stylus Primacy (Absolute Nullifier Burning):**
   Setiap spend privat yang melibatkan penarikan dana atau pembayaran merchant—baik lokal maupun cross-chain—**WAJIB** mengeksekusi verifikasi bukti Groth16 dan membakar nullifier di Arbitrum Stylus terlebih dahulu:
   $$\text{note\_nullifiers}[\text{nullifier}] \leftarrow \text{true}$$
   Tidak ada jalur kode yang diizinkan memproses pengiriman lintas rantai jika transaksi di Stylus gagal atau tidak dieksekusi.
2. **Siloed Multi-Asset Solvency Invariant (DEC-034B / DEC-038B):**
   $$\text{Balance}(\text{Asset}_a) \ge \text{user\_note\_liabilities}[a] + \text{accrued\_execution\_fee\_liabilities}[a] + \text{realized\_protocol\_fees}[a]$$
   Setiap mutasi dana wajib menjaga kondisi solvabilitas ini secara deterministik.
3. **Chainlink CCIP v2 Canonical Standards:**
   - Transportasi chain selector wajib `uint64` (diserialisasi sebagai string di API/JSON).
   - Pengkodean `extraArgs` mengikuti standar CCIP 2.0 (Generic ExtraArgs V2/V3) dengan default full finality (`WAIT_FOR_FINALITY_FLAG = 0x00000000`).
   - Query monitoring menggunakan Chainlink CCIP v2 Official API (`https://api.ccip.chain.link/v2/messages/{messageId}`) tanpa kebutuhan API key untuk endpoint publik.
4. **Zero-Custody Compliance (DEC-054):**
   Relayer tidak pernah memegang kunci privat pengguna dan tidak pernah menjadi kustodian perantara dana pengguna.
5. **Rasio Pengujian Mandatori $\ge 2\times$:**
   Setiap pengujian sukses (positive test) wajib disertai minimal 2 pengujian kegagalan adversarial (negative tests) yang membuktikan kondisi fail-closed dan nol mutasi state.

---

## 3. Arsitektur Solusi & Komparasi Desain

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                   ARSITEKTUR LENGKAP CCIP PROTOKOL ZELTRA (DEC-056)                    │
├────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│   [ INFLOW RAIL (DEPOSIT) - DEC-055 ]                                                  │
│   Pengguna di Ethereum / Base / Polygon                                                │
│         │                                                                              │
│         ▼                                                                              │
│   ccipSend(Arbitrum, Receiver=Zeltra, Token=[USDC/WETH], Data=[32B CM || 96B Memo])   │
│         │                                                                              │
│         ▼ (Chainlink DON)                                                              │
│   zeltra.ccipReceive(Any2EVMMessage) di Arbitrum Stylus                                │
│         │                                                                              │
│         ├──> Validasi CM (< r Fr scalar) & 0 bps deposit fee                           │
│         ├──> _mmr_insert(CM, Memo) => Emit NoteCommitmentAppended                      │
│         └──> Emit CCIPDepositExecuted(messageId, leafIndex, CM)                        │
│                                                                                        │
│   ───────────────────────────────────────────────────────────────────────────────────  │
│                                                                                        │
│   [ OUTFLOW RAIL (SPEND CROSS-CHAIN) - SANITASI DEC-056 ]                              │
│   Pengguna membuat Groth16 Proof (Mode 3 JoinSplit / NoteCircuit)                      │
│         │                                                                              │
│         ▼                                                                              │
│   Relayer / Self-Broadcast: spend_joinsplit() di Arbitrum Stylus                       │
│         │                                                                              │
│         ├──> Verifikasi Groth16 Proof via EIP-2537                                     │
│         ├──> BAKAR NULLIFIER on-chain (Double-spend musnah!)                           │
│         ├──> Potong liabilitas & cairkan token ke payout gateway                       │
│         └──> BARU relayer memanggil ccipSend ke Router dengan TOKEN AKTUAL             │
│                                                                                        │
│   ───────────────────────────────────────────────────────────────────────────────────  │
│                                                                                        │
│   [ UNIFIED LIFECYCLE MONITOR (zeltra-node) ]                                          │
│   ccip_monitor_loop memantau Inbound Deposit & Outbound Spend:                         │
│         │                                                                              │
│         ├──> Query Chainlink CCIP v2 API: GET /v2/messages/{messageId}                │
│         │    Status: SENT -> VERIFYING/COMMITTED -> SUCCESS / FAILED                   │
│         ├──> Query Arbitrum Stylus Contract: ccip_processed_messages(messageId)        │
│         └──> Update Database SQLite: mmr_leaves & spend_queue                          │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Spesifikasi Teknis Implementasi

### A. Eliminasi Bug di Smart Contract (`zeltra-contracts/src/spend.rs`)

Fungsi peninggalan legacy:
```rust
// HAPUS DARI spend.rs (Telah dipindahkan secara kanonikal ke ccip_deposit.rs):
// pub fn _ccip_receive(&mut self, message_id, source_chain_selector, sender, _payload)
```
Seluruh penanganan CCIP inbound ditangani secara eksklusif oleh `zeltra-contracts/src/ccip_deposit.rs` melalui fungsi:
```rust
pub fn _ccip_receive_programmable(&mut self, message: CcipAny2EVMMessage) -> Result<(), Vec<u8>>
```

### B. Eliminasi Bypass di Relayer (`zeltra-node/src/batch_processor.rs`)

Cabang bypass lama:
```rust
// KODE LAMA BERBAHAYA (DIHAPUS):
// else if let Some(cc) = &request.cross_chain {
//     evm_client.broadcast_ccip_transaction(...) // BYPASS ARBITRUM!
// }
```

Digantikan dengan mekanisme **Arbitrum Verification First**:
```rust
// KODE BARU AMAN:
// 1. Setiap spend request WAJIB memiliki bukti ZK yang sah dan dieksekusi di Arbitrum:
let outcome = if request.is_joinsplit() {
    evm_client.broadcast_spend_joinsplit_transaction(...).await?
} else if request.has_zk_proof() {
    evm_client.broadcast_spend_private_note_transaction(...).await?
} else {
    // Fail-Closed: Tolak spend tanpa bukti ZK sah di Arbitrum!
    self.db.fail_spend(item.id, None, "unsupported spend type: valid ZK proof required on Arbitrum").await?;
    return;
};

// 2. Jika transaksi di Arbitrum BERHASIL dan terdapat parameter cross_chain:
if outcome.success {
    if let Some(cc) = &request.cross_chain {
        // Eksekusi penyiaran CCIP outflow dengan token nyata setelah nullifier dibakar on-chain
        match evm_client.broadcast_ccip_outflow(cc, request.amount).await {
            Ok(ccip_outcome) => {
                self.db.set_ccip_tracking(item.id, &ccip_outcome.message_id, cc.destination_chain_selector).await?;
            }
            Err(e) => {
                // Catat kegagalan dispatch cross-chain
                self.db.update_destination_status(item.id, "dispatch_failed", Some(&e.to_string())).await?;
            }
        }
    }
}
```

### C. Sanitasi Token Outflow & CCIP v2 ExtraArgs (`zeltra-node/src/evm_client.rs`)

Pada `broadcast_ccip_transaction`:
1. `tokenAmounts` dikonfigurasi dengan alamat token nyata dan nominal transfer.
2. `extraArgs` dikonfigurasi dengan standar CCIP 2.0:
   - Tag V2: `0x181dcf10` (`EVMExtraArgsV2 { gasLimit, allowOutOfOrderExecution: true }`).
   - Tag V3: `0x4430e38a` (`GenericExtraArgsV3`).
3. Dilakukan verifikasi `isChainSupported(destinationChainSelector)` pada Router sebelum melakukan `getFee` atau `ccipSend`.

### D. Arsitektur Unified CCIP Lifecycle Monitor (`zeltra-node/src/ccip_monitor.rs`)

Monitor berjalan sebagai background task di `zeltra-node` yang menangani dua alur secara serentak:

#### 1. Inbound Deposit Monitor:
- Mengambil daftar deposit CCIP yang belum terkonfirmasi di pool dari database.
- Memeriksa status di Chainlink CCIP v2 API:
  `GET https://api.ccip.chain.link/v2/messages/{messageId}`
- Memeriksa status on-chain di Stylus:
  `zeltra.ccip_processed_messages(messageId)`.
- Jika `true`, mengaitkan `messageId` dengan leaf index di `mmr_leaves` dan menandai status sebagai `CONFIRMED_IN_POOL`.

#### 2. Outbound Spend Monitor:
- Mengambil transaksi spend di `spend_queue` yang memiliki `destination_status = 'pending'`.
- Memeriksa status di Chainlink CCIP v2 API:
  - `status == "SUCCESS"` $\implies$ Update `destination_status = 'delivered'`.
  - `status == "FAILED"` $\implies$ Update `destination_status = 'failed'`, simpan `destination_error`.
  - Status in-flight (`SENT`, `VERIFYING`, `COMMITTED`, `BLESSED`) $\implies$ Perbarui timestamp, tunggu siklus berikutnya dengan exponential backoff.

### E. Public REST API Endpoints (`zeltra-node`)

1. `GET /api/v1/ccip/deposit/:message_id`
   - Mengembalikan status deposit inbound:
     ```json
     {
       "message_id": "0x...",
       "status": "CONFIRMED_IN_POOL",
       "source_chain_selector": "16015286601757825753",
       "leaf_index": 42,
       "note_commitment": "0x...",
       "block_number": 1234567,
       "delivery_time_ms": 45200
     }
     ```
2. `GET /api/v1/ccip/spend/:message_id`
   - Mengembalikan status spend outflow:
     ```json
     {
       "message_id": "0x...",
       "status": "DELIVERED",
       "destination_chain_selector": "10344971235874465080",
       "receipt_tx_hash": "0x...",
       "error": null
     }
     ```
3. `GET /api/v1/ccip/status/:message_id`
   - Universal status endpoint untuk memantau pesan CCIP apapun (deposit maupun spend).

---

## 5. Rencana Verifikasi Empiris & Test Gates

Sesuai aturan baku protokol: **$\text{Negative Tests} \ge 2 \times \text{Positive Tests}$** dengan pembuktian nol mutasi state saat gagal.

- **Gate 1 (Positive Tests):**
  - Verifikasi spend Arbitrum dengan parameter cross-chain berhasil membakar nullifier di Arbitrum.
  - Verifikasi parser respons Chainlink CCIP v2 API untuk status `SUCCESS`, `VERIFYING`, `SENT`, dan `FAILED`.
  - Verifikasi query endpoint `/api/v1/ccip/deposit/:id` dan `/api/v1/ccip/spend/:id`.
- **Gate 2 (Negative & Security Tests — Minimal 2x):**
  - Penolakan spend request cross-chain yang tidak memiliki bukti ZK yang sah (`fail-closed`).
  - Pembuktian bahwa kegagalan pengiriman CCIP outbound **TIDAK MENGHAPUS** nullifier yang sudah terbakar di on-chain Arbitrum (Anti-Revocation).
  - Penolakan request dengan selector rantai yang tidak didukung atau router tidak terdaftar.
  - Penanganan gracefully saat Chainlink CCIP API mengembalikan `404 Not Found` (fresh unindexed message) dengan backoff tanpa crash.
  - Penolakan replay `messageId` deposit ganda.
- **Gate 3 (Solvency & Parity Invariant):**
  - Pembuktian bahwa seluruh saldo pool token pada smart contract Arbitrum tetap solvent dan seimbang setelah rangkaian spend dan deposit CCIP.
- **Gate 4 (Clean Compilation & Clippy):**
  - Workspace compiles with 0 errors and passes all automated test suites.

---

## 6. Referensi & Sitasi Akademik

1. **Chainlink Labs (2025–2026):** *"Chainlink Cross-Chain Interoperability Protocol (CCIP) v1.6 & v2.0 Architecture"*, [`https://docs.chain.link/ccip`](https://docs.chain.link/ccip).
2. **Chainlink Labs (2026):** *"CCIP API Specification v2.13.0"*, [`https://api.ccip.chain.link/docs`](https://api.ccip.chain.link/docs).
3. **DEC-055 (Zeltra Architecture):** [`DEC-055-cross-chain-ccip-programmable-inbound-deposit-and-mmr-minting.md`](DEC-055-cross-chain-ccip-programmable-inbound-deposit-and-mmr-minting.md).
4. **DEC-043B (Zeltra Architecture):** [`DEC-043B-strict-three-mode-joinsplit-and-malleability-elimination.md`](DEC-043B-strict-three-mode-joinsplit-and-malleability-elimination.md).
5. **DEC-054 (Zeltra Architecture):** [`DEC-054-zero-custody-safe-harbor-and-direct-self-broadcast.md`](DEC-054-zero-custody-safe-harbor-and-direct-self-broadcast.md).
