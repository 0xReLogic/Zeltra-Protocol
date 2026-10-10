# DEC-036: Universal 2-in-2-out JoinSplit End-to-End Architecture (Circuit, SDK, Relayer, Smart Contract, and MMR)

- **Status:** APPROVED AS FOUNDER BLUEPRINT — ZERO TECH DEBT (Direction B Ratified, In-Pool Consolidation Ratified, Paginated Bulk Sync Ratified)
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-core/src/joinsplit_circuit.rs` (Universal 2-in-2-out JoinSplit R1CS circuit, MMR bagging, dual-epoch nullifier PRF, two-limb quote hash)
  - `nimbus-sdk/src/wallet/note_wallet.rs` (Multi-note coin selection, dual-witness generation, MMR proof sync, JoinSplit spend API)
  - `nimbus-node/src/handlers/spend.rs` (Relayer `/api/v1/spend-joinsplit` endpoint, dual-nullifier double-spend guard, preflight verifier)
  - `nimbus-contracts/src/spend.rs` (Stylus on-chain `spend_joinsplit` entrypoint, dual-nullifier state update, Direction B dual-output MMR insertion)
  - `nimbus-contracts/src/groth16_joinsplit_verifier.rs` (EIP-2537 MSM and Pairing verifier routines for JoinSplit)
- **Child Sub-Specifications:**
  - [`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md) — Universal 2-in-2-out JoinSplit R1CS Circuit, Strict Constant-Arity Topology, and 19-Public-Input Soundness Specification (`nimbus-core`)
  - [`DEC-036B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md) — Client SDK Multi-UTXO Knapsack Coin Selection, In-Pool Progressive Consolidation, and Anti-Snooping MMR Tree Sync (`nimbus-sdk`)
  - [`DEC-036C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036C-relayer-ingress-guard-and-stylus-joinsplit-settlement.md) — Relayer Ingress Pipeline, Dual-Nullifier Guard, and Stylus Smart Contract 2-in-2-out Settlement with EIP-2537 (`nimbus-node` & `nimbus-contracts`)
- **Related DECs:**
  - [`DEC-016`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) (Private Note Change Ledger Concept)
  - [`DEC-030`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-030-multi-utxo-joinsplit-coin-selection-zeroize-and-aead-backup.md) (Multi-UTXO JoinSplit Prototype & Knapsack Coin Selection)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) (Merkle Mountain Range Commitment Accumulator)
  - [`DEC-033`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md) (Epoch-Windowed Nullifier Registry & In-Flight Rollover)
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Master Cryptographic Canonicality & Blueprint)
  - [`DEC-035A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md) (Two-Limb Quote Hash Sub-Spec)
  - [`DEC-035B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035B-formal-scope-binding-gadget-and-groth16-statement-integrity.md) (Formal Scope Binding Gadget Sub-Spec)
  - [`DEC-035C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md) (Relayer MMR Indexer & Sync Sub-Spec)

---

## 1. Executive Summary & Problem Statement

Pada arsitektur single-note (`PrivateNoteCircuit` 1-in-1-out), pengguna hanya dapat membelanjakan 1 note tunggal yang nilainya $\ge \text{total\_required}$. 
Jika seorang pengguna memiliki total saldo **$10.00 USDC** yang terpecah dalam dua note @**$5.00 USDC**, pembayaran sebesar **$7.00 USDC** akan **gagal dengan error `InsufficientBalance`**, mengunci daya beli pengguna (*balance fragmentation lockout*).

Dokumen **DEC-036** (Master Blueprint) beserta ketiga sub-spesifikasinya ([`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md), [`DEC-036B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md), dan [`DEC-036C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036C-relayer-ingress-guard-and-stylus-joinsplit-settlement.md)) menetapkan arsitektur komprehensif implementasi **Universal 2-in-2-out JoinSplit** dari sirkuit R1CS hingga smart contract on-chain sebelum **MPC Trusted Setup Ceremony** dilakukan.

### Pilar Utama Arsitektur JoinSplit:
1. **Pencegahan Fragmentasi Saldo 2-Note:** Pengguna dapat menggabungkan 2 pecahan note menjadi pembayaran tunggal plus kembalian secara atomik dalam satu transaksi.
2. **Direction B: Strict Constant-Arity Topology (Ratifikasi Founder):** Menyeragamkan seluruh transaksi spend on-chain menjadi tepat **2 nullifier dibakar** dan **2 daun baru disisipkan ke MMR**. Jika pengguna hanya membelanjakan 1 note atau hanya membutuhkan 1 change note, slot kedua diisi dengan *Canonical Dummy Zero-Note*, meniadakan seluruh kebocoran metadata ariti transaksi (Zcash ZIP 315).
3. **Penyatuan Setup Phase 2 Groth16:** Penggunaan sirkuit tunggal universal meminimalkan kebutuhan ceremony Groth16 Phase 2 menjadi satu setup terpadu untuk sirkuit spend saat freeze, daripada membagi ceremony ke berbagai varian ariti spend.

---

## 2. Invariants

Sistem JoinSplit wajib memenuhi invarian berikut secara mutlak di seluruh lapisan:

- **INV-1 (Exact Value Conservation & Wrap-Around Immunity):**
  $$(v_{in,1} + v_{in,2}) = (v_{\text{merchant}} + v_{\text{protocol\_fee}} + v_{\text{execution\_fee}}) + (v_{out,1} + v_{out,2})$$
  Seluruh 7 nilai moneter di-decompose menjadi 64 boolean bits ($7 \times 64 = 448$ boolean range constraints), menjamin $\sum V \ll r$ dan meniadakan risiko overflow/wrap-around di $\mathbb{F}_r$.
- **INV-2 (Direction B Constant-Arity Topologic Privacy):**
  Setiap transaksi `spend_joinsplit` wajib membakar tepat 2 nullifier dan menyisipkan tepat 2 daun komitmen ke MMR on-chain.
- **INV-3 (Public Input Statement Binding):**
  Seluruh 19 public inputs dialokasikan dan terikat ke constraint R1CS non-trivial sedemikian rupa sehingga verifying key element $IC[0..19] \ne \mathcal{O}$.
- **INV-4 (Dual-Nullifier Non-Aliasing & Double-Spend Defense):**
  $$\text{nf}_1 \ne \text{nf}_2 \quad \land \quad \text{spent}[\text{nf}_1] == \text{false} \quad \land \quad \text{spent}[\text{nf}_2] == \text{false}$$
  ditegakkan fail-closed di layer relayer dan smart contract.
- **INV-5 (Mathematical Solvency):**
  $$\text{Contract USDC Balance} \ge \text{Total Liabilities}$$
  ditegakkan pada akhir setiap eksekusi state change.

---

## 3. Struktur Spesifikasi Modular (DEC-036 Architecture Suite)

Arsitektur Universal 2-in-2-out JoinSplit dipisahkan secara modular ke dalam 3 sub-spesifikasi teknis mendalam:

```
                            DEC-036 MASTER BLUEPRINT
           Universal 2-in-2-out JoinSplit End-to-End Architecture
                                      │
         ┌────────────────────────────┼────────────────────────────┐
         │                            │                            │
         ▼                            ▼                            ▼
     DEC-036A                     DEC-036B                     DEC-036C
[R1CS Circuit & Soundness]   [Client SDK & Sync]         [Relayer & Contract]
- 7x 64-bit Range Checks     - Knapsack Coin Selection   - Relayer /api/v1 Ingress
- Direction B Constant Arity - In-Pool Consolidation     - Dual-Nullifier Guards
- Dummy Second Input Gadget  - Paginated Bulk Sync       - Stylus spend_joinsplit
- MMR Peak Bagging Gadget    - Crash-Safe 2PC Storage    - EIP-2537 MSM & Pairing
- 19 Public Inputs Layout    - Dual-Epoch Witness Sync   - Arb Sepolia Target Active
```

### Ringkasan Cakupan Sub-Spesifikasi:

1. **[`DEC-036A: Universal 2-in-2-out JoinSplit R1CS Circuit Specification`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md):**
   - Mendefinisikan persamaan konservasi nilai, 448 boolean range constraints anti wrap-around, dan formula protocol fee 45 bps ceiling division.
   - Gadget Canonical Dummy Second Input (`is_dummy_2`, zero value, conditional MMR check, domain-separated dummy nullifier).
   - Gadget Direction B Canonical Dummy Output Commitment ($v=0$).
   - Gadget in-circuit MMR Peak Bagging (domain 3, hingga 32 puncak).
   - Integrasi Two-Limb 128-bit Quote Hash (DEC-035A) dan Formal Scope Binding (DEC-035B).
   - Definisi dan urutan 19 Public Inputs Kanonikal beserta anggaran ~10.800 R1CS constraints.

2. **[`DEC-036B: Client SDK Knapsack Coin Selection, Consolidation, and Bulk Sync`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md):**
   - Algoritma pemilihan koin 4-tingkat (*Stochastic Knapsack Optimization*) untuk 1 atau 2 note dengan peminimalan debu kembalian.
   - Arsitektur **In-Pool Progressive Consolidation** (`consolidate_notes`) untuk menangani fragmentasi saldo ekstrem ($N > 2$ notes) tanpa memperbesar constraint sirkuit.
   - Mesin **Anti-Snooping Paginated Bulk MMR Tree Sync** (`GET /api/v1/mmr/leaves?from={idx}&limit=1000`) yang merekonstruksi pohon di sisi klien (WASM/CPU) tanpa membocorkan indeks daun kepada relayer.
   - Dual-witness generation lintas epoch ($E$ dan $E-1$) serta manajemen siklus hidup 2PC crash-safe note storage.

3. **[`DEC-036C: Relayer Ingress Pipeline, Dual-Nullifier Guard, and Stylus Settlement`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036C-relayer-ingress-guard-and-stylus-joinsplit-settlement.md):**
   - Pipeline ingress relayer `POST /api/v1/spend-joinsplit` dengan validasi skalar kanonikal ($< r$), pencegahan nullifier self-aliasing (`nf_1 != nf_2`), dan dual-layer double-spend guard (SQLite & RPC view).
   - Preflight local Groth16 CPU verifier untuk mencegah serangan pengurasan gas relayer (*gas griefing*).
   - Entrypoint Stylus WASM `spend_joinsplit` dengan verifikasi EIP-2537 (`0x0c` MSM 20 titik + `0x0f` 4-pairing check).
   - Eksekusi topologi konstan on-chain: selalu membakar 2 nullifier dan menyisipkan tepat 2 daun ke MMR on-chain (`_mmr_insert` dua kali).
   - Verifikasi status precompile jaringan target (Arbitrum Sepolia `421614` aktif vs Arbitrum One `42161` gated).

---

## 4. Forensic Gap Analysis: Status Aktual Repository

Kondisi implementasi JoinSplit di dalam codebase Zeltra saat ini:

| Layer | Komponen Codebase | Status Aktual | Gap / Masalah yang Wajib Diselesaikan |
|---|---|---|---|
| **Layer 1: Circuit** | `nimbus-core/src/joinsplit_circuit.rs` | **Prototipe Parsial (40%)** | 1. Implementasikan MMR bagging gadget (DEC-032 / DEC-036A).<br>2. Ikat `epoch_id_1` dan `epoch_id_2` ke derivasi nullifier (DEC-033).<br>3. Terapkan Two-Limb 128-bit quote hash & scope binding (DEC-035 / DEC-036A).<br>4. Terapkan Direction B Canonical Dummy Output gadget. |
| **Layer 2: SDK / Wallet** | `nimbus-sdk/src/wallet/note_wallet.rs` | **Prototipe Parsial (50%)** | 1. Implementasikan `consolidate_notes` untuk $N > 2$ (DEC-036B).<br>2. Integrasikan bulk leaf downloader anti-snooping (`/api/v1/mmr/leaves`).<br>3. Tambahkan dual-epoch witness synthesis.<br>4. Sediakan HTTP client dispatcher untuk JoinSplit spend. |
| **Layer 3: Relayer** | `nimbus-node/src/handlers/spend.rs` | **0% (Kosong Total)** | 1. Buat endpoint `POST /api/v1/spend-joinsplit` (DEC-036C).<br>2. Implementasikan dual-nullifier double-spend check di SQLite & on-chain.<br>3. Tambahkan local CPU Groth16 preflight verification.<br>4. Implementasikan broadcast transaction dispatcher JoinSplit. |
| **Layer 4: Smart Contract** | `nimbus-contracts/src/spend.rs` | **0% (Kosong Total)** | 1. Buat entrypoint `spend_joinsplit` di Stylus WASM (DEC-036C).<br>2. Implementasikan verifier Groth16 EIP-2537 20-titik MSM & 4-pairing.<br>3. Terapkan Direction B dual-leaf MMR insertion konstan.<br>4. Terapkan dual-nullifier burning dan invariant check. |
| **Layer 5: MMR Accumulator** | `nimbus-core` & `nimbus-contracts` | **Parsial (Sisi Circuit Bolong)** | Smart contract sudah memiliki `_mmr_insert`, hubungkan dengan sirkuit `JoinSplitCircuit` melalui in-circuit MMR peak bagging gadget. |

---

## 5. Ratifikasi Keputusan Eksekutif Founder (Executive Consensus)

Berikut keputusan eksekutif resmi yang telah diratifikasi oleh Founder:

1. **Aktivasi Precompile Jaringan Target:**
   - **Keputusan:** Fokus 100% pada **Arbitrum Sepolia (`421614`)** untuk fase pra-testnet Gate G. Arbitrum One dipending hingga jadwal mainnet ditetapkan oleh tata kelola.
2. **Privasi Topologi Output (Direction B Diadopsi):**
   - **Keputusan:** Mengadopsi **Direction B (Strict Constant-Arity Topology)** secara permanen. Kontrak Stylus **selalu menyisipkan tepat 2 daun ke MMR** pada setiap transaksi spend (`_mmr_insert(cm1)` dan `_mmr_insert(cm2)`). Jika pengguna tidak membutuhkan kembalian kedua, sirkuit menghasilkan *Canonical Dummy Output Commitment* ($v=0$). Tambahan gas ~2.100 gas (<$0.0001 / Rp 0,8) disetujui demi mencapai privasi topologi 10/10 yang tidak bisa dianalisis pihak luar.
3. **Penanganan Fragmentasi Saldo Ekstrem ($N > 2$):**
   - **Keputusan:** Mengadopsi **In-Pool Progressive Consolidation** via `consolidate_notes` di SDK. Pengguna yang memiliki $\ge 3$ note kecil dapat menggabungkannya secara bertahap di dalam shielded pool hanya dengan membayar biaya gas relayer minimal, menjaga sirkuit tetap ramping (~10.800 constraints) dan proving latency di bawah 2.5 detik.
4. **Privasi Sinkronisasi MMR (Paginated Bulk/Range Sync):**
   - **Keputusan:** Mengadopsi **Paginated Bulk/Range Sync**. Relayer melayani penarikan daun komitmen secara chunking (misal 1.000 daun per request = hanya ~32 KB). Dompet menyusun dan memverifikasi jalur MMR secara lokal (client-side), sehingga operator relayer sama sekali tidak dapat memetakan note mana yang dimiliki oleh pengguna.

---

## 6. Analisis Upacara Setup Groth16 (Koreksi Klaim MPC Ceremony)

Penting untuk membedakan struktur upacara setup Groth16:
1. **Phase 1 (Powers of Tau):** Bersifat universal dan independen terhadap sirkuit. Parameter Phase 1 berukuran besar (misalnya $2^{20}$ s/d $2^{24}$ powers) dapat digunakan kembali dari upacara publik yang telah diaudit secara global (misal Hermez / Perpetual Powers of Tau).
2. **Phase 2 (Circuit-Specific Setup):** Bersifat terikat secara ketat pada sistem constraint R1CS sirkuit spesifik.
3. **Koreksi Terhadap Klaim "Single Ceremony":**
   - Mengadopsi JoinSplit sebagai satu-satunya sirkuit spend protokol berarti hanya diperlukan **satu kali upacara Phase 2 untuk modul spend saat pembekuan (circuit freeze)**.
   - Namun, hal ini **TIDAK** menjamin satu ceremony untuk seumur hidup protokol. Setiap modifikasi terhadap constraint, penambahan public input, atau penambahan sirkuit baru (misal sirkuit compliance masa depan) akan memerlukan upacara Phase 2 yang baru.

---

## 7. Roadmap & Integrasi Menuju Testnet Gate G

Sesuai arahan Founder (Zero Tech Debt):
1. **Fase 1 (DEC-035):** Selesaikan hardening sirkuit 1-in-1-out, Two-Limb quote decomposition, scope binding gadget, dan MMR indexer agar pipeline inti terbukti lolos E2E di Arbitrum Sepolia.
2. **Fase 2 (DEC-036 Suite):** Terapkan ekstensi JoinSplit 2-in-2-out secara terstruktur melintasi 5 layer ([`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md), [`DEC-036B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md), dan [`DEC-036C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036C-relayer-ingress-guard-and-stylus-joinsplit-settlement.md)) setelah baseline testnet stabil dan sebelum upacara MPC Phase 2 digelar.

---

## 8. External References and Standards

1. **Arbitrum Nitro ArbOS 40 Release:** [https://docs.arbitrum.io/run-a-node/arbos-releases/arbos40](https://docs.arbitrum.io/run-a-node/arbos-releases/arbos40)
2. **Arbitrum Nitro ArbOS 51 Release:** [https://docs.arbitrum.io/run-a-node/arbos-releases/arbos51](https://docs.arbitrum.io/run-a-node/arbos-releases/arbos51)
3. **EIP-2537 Precompile Specification:** [https://eips.ethereum.org/EIPS/eip-2537](https://eips.ethereum.org/EIPS/eip-2537)
4. **Zcash ZIP 315 (Best Practices for Wallet Implementations):** [https://zips.z.cash/zip-0315](https://zips.z.cash/zip-0315)
5. **Circom Proving Circuits Documentation:** [https://docs.circom.io/getting-started/proving-circuits/](https://docs.circom.io/getting-started/proving-circuits/)
6. **Filecoin Phase 2 Attestations Repository:** [https://github.com/filecoin-project/phase2-attestations](https://github.com/filecoin-project/phase2-attestations)
