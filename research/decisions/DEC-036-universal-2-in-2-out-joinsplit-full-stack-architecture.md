# DEC-036: Universal 2-in-2-out JoinSplit End-to-End Architecture (Circuit, SDK, Relayer, Smart Contract, and MMR)

- **Status:** APPROVED AS FOUNDER BLUEPRINT — ZERO TECH DEBT (Direction B Ratified, In-Pool Consolidation Ratified, Paginated Bulk Sync Ratified)
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-core/src/joinsplit_circuit.rs` (Universal 2-in-2-out JoinSplit R1CS circuit, MMR bagging, dual-epoch nullifier PRF, two-limb quote hash)
  - `nimbus-sdk/src/wallet/note_wallet.rs` (Multi-note coin selection, dual-witness generation, MMR proof sync, JoinSplit spend API)
  - `nimbus-node/src/handlers/spend.rs` (Relayer `/api/v1/spend-joinsplit` endpoint, dual-nullifier double-spend guard, preflight verifier)
  - `nimbus-contracts/src/spend.rs` (Stylus on-chain `spend_joinsplit` entrypoint, dual-nullifier state update, dual-output MMR insertion)
  - `nimbus-contracts/src/groth16_joinsplit_verifier.rs` (EIP-2537 MSM and Pairing verifier routines for JoinSplit)
- **Related DECs:**
  - [`DEC-016`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) (Private Note Change Ledger Concept)
  - [`DEC-030`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-030-multi-utxo-joinsplit-coin-selection-zeroize-and-aead-backup.md) (Multi-UTXO JoinSplit Prototype & Knapsack Coin Selection)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) (Merkle Mountain Range Commitment Accumulator)
  - [`DEC-033`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md) (Epoch-Windowed Nullifier Registry & In-Flight Rollover)
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Cryptographic Canonicality, Two-Limb Quote Hash, Scope Binding Gadget)

---

## 1. Executive Summary & Problem Statement

Pada arsitektur single-note (`PrivateNoteCircuit` 1-in-1-out), pengguna hanya dapat membelanjakan 1 note tunggal yang nilainya $\ge \text{total\_required}$. 
Jika seorang pengguna memiliki total saldo **$10.00 USDC** yang terpecah dalam dua note @**$5.00 USDC**, pembayaran sebesar **$7.00 USDC** akan **gagal dengan error `InsufficientBalance`**, mengunci daya beli pengguna (*balance fragmentation lockout*).

Dokumen **DEC-036** menetapkan spesifikasi arsitektur komprehensif implementasi **Universal 2-in-2-out JoinSplit** dari sirkuit R1CS hingga smart contract on-chain sebelum **MPC Trusted Setup Ceremony** dilakukan.

Prinsip Utama Arsitektur:
1. **Pencegahan Fragmentasi Saldo 2-Note:** Pengguna dapat menggabungkan 2 pecahan note menjadi pembayaran tunggal plus kembalian secara atomik dalam satu transaksi.
2. **Fixed Input-Slot Topology:** Transaksi 1-in disamarkan ke dalam 2 slot input menggunakan *Canonical Dummy Zero-Note*, menyeragamkan jumlah nullifier yang diproses on-chain (selalu 2 nullifier).
3. **Penyatuan Setup Phase 2 Groth16:** Penggunaan sirkuit tunggal universal meminimalkan kebutuhan ceremony Groth16 Phase 2 menjadi satu setup terpadu untuk sirkuit spend saat freeze, daripada membagi ceremony ke berbagai varian ariti spend.

---

## 1A. Prasyarat Deployment & Verifikasi EIP-2537 Jaringan Target

Eksekusi verifikasi Groth16 BLS12-381 pada Arbitrum Stylus mengandalkan host precompiles EIP-2537:
- `0x0c`: BLS12-381 G1 Multi-Scalar Multiplication (MSM)
- `0x0f`: BLS12-381 Pairing Check

### Status Aktivasi Jaringan:
1. **Target Testnet: Arbitrum Sepolia (`chain_id: 421614`):**
   - **Status:** **VERIFIED & ACTIVE**. Arbitrum Sepolia menjalankan ArbOS 40+ yang telah mengaktifkan precompile EIP-2537 (`0x0b` s/d `0x12`). Uji verifikasi pairing baseline 13/13 negative test telah berhasil dieksekusi di Sepolia.
2. **Target Mainnet: Arbitrum One (`chain_id: 42161`):**
   - **Status:** **BLOCKED — REQUIRES CHAIN VERIFICATION**. Aktivasi EIP-2537 di Arbitrum One bergantung pada rilis dan adopsi ArbOS 40/ArbOS 51 oleh validator governance. Verifikasi ketersediaan precompile pada node Arbitrum One wajib dibuktikan sebelum deployment mainnet.
3. **Acceptance Criteria Pra-Deploy:**
   - Contract test probe wajib mengeksekusi panggilan simulasi statis ke `0x0c` dan `0x0f` dengan pasangan titik dummy terverifikasi pada target network RPC sebelum mendaftarkan verifying key JoinSplit ke contract registry.

---

## 2. Forensic Gap Analysis: Status Aktual Repository

Kondisi implementasi JoinSplit saat ini di dalam codebase Zeltra:

| Layer | Komponen Codebase | Status Aktual | Gap / Masalah yang Wajib Diselesaikan |
|---|---|---|---|
| **Layer 1: Circuit** | `nimbus-core/src/joinsplit_circuit.rs` | **Prototipe Parsial (40%)** | 1. Masih menggunakan LeanIMT depth 20 statis, belum MMR bagging (DEC-032).<br>2. Derivasi nullifier belum mengikat `epoch_id` (DEC-033).<br>3. Quote hash masih single scalar, belum Two-Limb 128-bit (DEC-035).<br>4. Scope binding `_binding` masih berupa *dead code*. |
| **Layer 2: SDK / Wallet** | `nimbus-sdk/src/wallet/note_wallet.rs` | **Prototipe Parsial (50%)** | 1. Algoritma Knapsack Coin Selection sudah ada.<br>2. Pembuatan proof lokal ada, namun jalur MMR path masih statis.<br>3. Belum ada handling jika Note 1 dan Note 2 berada di Epoch berbeda.<br>4. Belum ada client HTTP dispatcher untuk memanggil relayer JoinSplit. |
| **Layer 3: Relayer** | `nimbus-node/src/handlers/spend.rs` | **0% (Kosong Total)** | 1. Tidak ada endpoint `/api/v1/spend-joinsplit`.<br>2. Tidak ada dual-nullifier double-spend check di SQLite & on-chain.<br>3. Tidak ada Groth16 preflight verification untuk JoinSplit.<br>4. Tidak ada broadcast transaction dispatcher untuk JoinSplit. |
| **Layer 4: Smart Contract** | `nimbus-contracts/src/spend.rs` | **0% (Kosong Total)** | 1. Tidak ada entrypoint `spend_joinsplit` di Stylus WASM.<br>2. Tidak ada verifier Groth16 rutinitas EIP-2537 untuk JoinSplit.<br>3. Tidak ada logika penandaan 2 nullifier sekaligus.<br>4. Tidak ada logika penyisipan 2 change note ke accumulator on-chain. |
| **Layer 5: MMR Accumulator** | `nimbus-core` & `nimbus-contracts` | **Parsial (Sisi Circuit Bolong)** | Smart contract sudah punya MMR on-chain (`_mmr_insert`), tetapi sirkuit `JoinSplitCircuit` belum memiliki gadget verifikasi MMR peak bagging. |

---

## 3. Spesifikasi Arsitektur JoinSplit 2-in 2-out

### A. Layer 1: R1CS Circuit Specification (`nimbus-core`)

#### 1. Persamaan Konservasi Nilai Mutlak & Formula Fee
Sirkuit menegakkan persamaan integer modulo $\mathbb{F}_r$:
$$\sum V_{\text{in}} \equiv \sum V_{\text{out}} \pmod r$$
$$(v_{in,1} + v_{in,2}) = (v_{\text{merchant}} + v_{\text{protocol\_fee}} + v_{\text{execution\_fee}}) + (v_{out,1} + v_{out,2})$$

- **Buktian Soundness Anti Wrap-Around:** Seluruh 7 variabel nilai ($v_{in,1}, v_{in,2}, v_{\text{merchant}}, v_{\text{protocol\_fee}}, v_{\text{execution\_fee}}, v_{out,1}, v_{out,2}$) di-decompose ke dalam **64 boolean bits** ($7 \times 64 = 448$ boolean range constraints via `enforce_u64_range`). Karena nilai maksimum akumulasi $7 \times (2^{64}-1) \approx 1.29 \times 10^{20} \ll r \approx 5.24 \times 10^{77}$, modular wrap-around dan *minting underflow* di $\mathbb{F}_r$ secara matematis mustahil terjadi.
- **Formula Protocol Fee (Flat 45 bps):**
  $$\text{protocol\_fee} = \left\lceil \frac{v_{\text{merchant}} \times 45}{10\,000} \right\rceil$$
  Pembagian menggunakan *ceiling division* untuk menjamin solvensi selalu memihak protokol. Nilai ini diverifikasi identik antara witness sirkuit dan kalkulasi smart contract on-chain.

#### 2. Semantik 2-in-2-out & Batasan Kemampuan
- **Semantik Transaksi:** Transaksi menghasilkan:
  1. **1 Public ERC-20 Payout:** Transfer ke `recipient` sebesar $v_{\text{merchant}}$.
  2. **Maksimal 2 Private Shielded Notes:** $v_{out,1}$ (change note utama) dan $v_{out,2}$ (change note sekunder untuk private split payment / denominasi terpisah). Jika tidak ada change sekunder, $v_{out,2} = 0$.
- **Batas Kemampuan Input ($N \le 2$):**
  Sirkuit 2-input JoinSplit hanya dapat mengonsumsi **maksimal 2 note input dalam 1 transaksi atomik**.
  Jika sebuah dompet memiliki tingkat fragmentasi tinggi ($N > 2$, misalnya sepuluh note @$10 untuk menarik $100), transaksi **TIDAK DAPAT** diselesaikan dalam 1 bukti JoinSplit. Dompet wajib melakukan konsolidasi bertahap (*progressive multi-tx consolidation*) menggunakan `consolidate_notes` atau memanfaatkan arsitektur batch spend multi-transaksi.

#### 3. Keterikatan Mutlak Nilai Witness ke Komitmen (Value-to-Commitment Binding)
Prover **TIDAK BISA** memalsukan nilai witness ($v_{in,1}$ atau $v_{in,2}$) karena setiap input diikat ke komitmen Poseidon W5:
$$\text{cm}_1 = \text{Poseidon\_W5}([v_{in,1}, \text{owner}_1, \rho_1, \text{rand}_1], \text{DOMAIN\_NOTE})$$
$$\text{cm}_2 = \text{Poseidon\_W5}([v_{in,2}, \text{owner}_2, \rho_2, \text{rand}_2], \text{DOMAIN\_NOTE})$$

Sirkuit memverifikasi bahwa $\text{cm}_1$ dan $\text{cm}_2$ (jika bukan dummy) memiliki jalur Merkle ke puncak MMR yang valid terhadap public input `note_root`.

#### 4. Spesifikasi Ketat Dummy Second Input
Untuk transaksi 1-input, slot kedua diisi dengan Canonical Dummy Input dengan constraint ketat:
1. **Boolean Constraint:** `is_dummy_2 * (1 - is_dummy_2) == 0`.
2. **Zero Value Constraint:** `is_dummy_2 * in2_value == 0`.
3. **Conditional MMR Membership:** `(1 - is_dummy_2) * (computed_peak_2 - expected_peak_2) == 0`.
4. **Domain-Separated Dummy Nullifier:**
   $$\text{nf}_2 = \text{Poseidon\_W5}([\text{nk}_2, \text{session\_nonce}, 0, 0], \text{DOMAIN\_DUMMY\_NULLIFIER})$$
   Dijamin pseudorandom, ber-entropi tinggi, non-zero, deterministik terhadap `session_nonce`, dan tidak dapat digunakan ulang.
5. **Epoch Alignment:** Untuk mencegah kebocoran status dummy melalui public input vector, `epoch_id_2` pada mode dummy wajib diisi sama dengan `epoch_id_1`.
6. **Uniform Constraint Synthesis:** Sistem menghasilkan jumlah constraint R1CS yang identik (10.800 constraints) tanpa percabangan sintesis di host language (Rust).

#### 5. Analisis Privasi Topologi (Zcash ZIP 315 Context)
- **Status Metadata Saat Ini (Direction A - Fixed Input-Slot Topology):**
  Desain saat ini menyeragamkan input menjadi tepat 2 nullifier on-chain. Namun, mengacu pada telaah metadata Zcash ZIP 315:
  - `flags_packed` (`has_change_1`, `has_change_2`) dan `is_rollover` diekspos sebagai public input.
  - Smart contract Stylus melakukan penyisipan MMR secara kondisional (`if has_change { _mmr_insert(...) }`).
  - **Konsekuensi:** Pengamat on-chain dapat mengamati apakah transaksi menghasilkan 0, 1, atau 2 daun baru di MMR. Privasi topologi hanya berlaku pada sisi input slots, bukan ariti output penuh.
- **Opsi Ekstensi (Direction B - Strict Constant Arity Topology):**
  Jika privasi output penuh disyaratkan, kontrak wajib selalu menyisipkan tepat 2 daun ke MMR pada setiap spend. Jika change 2 tidak digunakan, sirkuit menghasilkan *Canonical Dummy Output Commitment* ($v=0$) yang disisipkan ke MMR dengan tambahan biaya gas (~2.100 gas). Keputusan ini dicatat sebagai open blocker arsitektur.

#### 6. MMR Accumulator Parameters & Domain Separation
- **Batas Pohon:** `max_leaf_count = 2^32 - 1` (maksimum kedalaman puncak MMR $\le 32$).
- **Domain Separation Constants:**
  - `DOMAIN_NOTE = 1`
  - `DOMAIN_MERKLE_NODE = 2`
  - `DOMAIN_MMR_BAG = 3`
  - `DOMAIN_NULLIFIER = 4`
  - `DOMAIN_DUMMY_NULLIFIER = 5`
  - `DOMAIN_QUOTE_BINDING = 6`
- **Dual-Input Binding:** Kedua input ($\text{cm}_1$ dan $\text{cm}_2$ real) wajib membuktikan keanggotaan pada `note_root` dan `leaf_count` kanonikal yang sama persis.

#### 7. Tata Letak Public Inputs Kanonikal (19 Elemen)
```text
Index 0:  note_root                  (32 bytes) - Bagged root MMR
Index 1:  leaf_count                 (32 bytes) - Total daun MMR saat ini
Index 2:  input_nullifier_1          (32 bytes) - Nullifier input note 1
Index 3:  input_nullifier_2          (32 bytes) - Nullifier input note 2 (atau dummy)
Index 4:  epoch_id_1                 (32 bytes) - Epoch ID untuk note 1
Index 5:  epoch_id_2                 (32 bytes) - Epoch ID untuk note 2 (disamakan jika dummy)
Index 6:  output_commitment_1        (32 bytes) - Change note 1
Index 7:  output_commitment_2        (32 bytes) - Change note 2
Index 8:  recipient                  (32 bytes) - Alamat merchant penerima
Index 9:  merchant_amount            (32 bytes) - Nominal pembayaran
Index 10: protocol_fee               (32 bytes) - Flat 45 bps protocol fee
Index 11: execution_fee              (32 bytes) - Relayer gas execution fee
Index 12: quote_hash_hi              (32 bytes) - 128-bit Big-Endian limb tinggi quote EIP-712
Index 13: quote_hash_lo              (32 bytes) - 128-bit Big-Endian limb rendah quote EIP-712
Index 14: chain_id                   (32 bytes) - Arbitrum chain ID target
Index 15: contract_address           (32 bytes) - Stylus smart contract target
Index 16: expiry                     (32 bytes) - Timestamp kadaluarsa quote
Index 17: flags_packed               (32 bytes) - Bit 0: has_change_1, Bit 1: has_change_2
Index 18: is_rollover                (32 bytes) - Flag rollover epoch
```

---

### B. Layer 2: Client SDK & Wallet Specification (`nimbus-sdk`)

1. **Dual-Note Coin Selection (Knapsack):**
   - Jika tersedia single note dengan saldo cukup $\rightarrow$ gunakan 1-note spend (input 2 diisi Canonical Dummy Zero-Note).
   - Jika tidak ada single note yang cukup, tetapi total saldo gabungan $\ge \text{target}$ $\rightarrow$ pilih 2 note dengan *Stochastic Knapsack Optimization*.
2. **Dual-Witness Generation & Dual-Epoch Sync:**
   - Memvalidasi bahwa kedua note berada pada `note_root` dan `leaf_count` yang sama sebelum membangkitkan proof.
   - Jika kedua note berasal dari epoch yang berbeda ($E$ dan $E-1$), wallet menyusun witness `epoch_id_1` dan `epoch_id_2` yang sesuai dengan masa aktif note masing-masing.
3. **Privasi Sinkronisasi MMR (Anti Relayer Snooping):**
   - **Risiko:** Endpoint single-leaf `GET /api/v1/mmr/proof/{leaf_index}` membocorkan indeks daun yang dimiliki pengguna kepada operator relayer.
   - **Solusi Mitigasi:** Wallet mendukung **Range/Bulk Tree Sync** (`GET /api/v1/mmr/leaves?from={idx}&to={idx}`). Dompet mengunduh komitmen daun secara berkelompok dan merekonstruksi pohon serta jalur pembuktian MMR secara lokal pada client CPU/WASM tanpa membocorkan indeks daun spesifik yang akan dibelanjakan.
4. **Dual Change Note Accounting:**
   - Simpan change note utama dan sekunder (jika aktif) ke database SQLite lokal wallet dalam status `NoteStatus::Unconfirmed` (2PC pattern DEC-024).

---

### C. Layer 3: Relayer Settlement Specification (`nimbus-node`)

1. **Endpoint Ingress:**
   - `POST /api/v1/spend-joinsplit`
2. **Preflight Dual-Nullifier Guard:**
   - Validasi bahwa `input_nullifier_1 != input_nullifier_2`.
   - Cek database SQLite lokal: kedua nullifier belum pernah tercatat.
   - Cek on-chain via simulation view: `is_nullifier_spent(nullifier_1) == false` DAN `is_nullifier_spent(nullifier_2) == false`.
3. **Fail-Closed Scalar Validation:**
   - Seluruh 19 public inputs diverifikasi `< r` via `from_evm_scalar`.
4. **Local CPU Groth16 Preflight:**
   - Verifikasi Groth16 proof pada CPU relayer sebelum memancarkan transaksi ke blockchain untuk mencegah *gas griefing*.

---

### D. Layer 4: Smart Contract Stylus Specification (`nimbus-contracts`)

1. **Entrypoint ABI:**
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
       quote_hash: FixedBytes<32>, // calldata bytes32 dipecah jadi hi & lo
       expiry: u64,
       flags_packed: u8,
   ) -> Result<(), Vec<u8>>
   ```
2. **On-Chain Dual-Nullifier Burning:**
   - Verifikasi `!self.note_nullifiers.get(nullifier_1)` dan `!self.note_nullifiers.get(nullifier_2)`.
   - Set `self.note_nullifiers.setter(nullifier_1).set(true)`.
   - Set `self.note_nullifiers.setter(nullifier_2).set(true)`.
3. **On-Chain MMR Output Insertion:**
   - Jika `has_change_1 == true` $\rightarrow$ panggil `_mmr_insert(output_cm_1)`.
   - Jika `has_change_2 == true` $\rightarrow$ panggil `_mmr_insert(output_cm_2)`.
4. **Verifikasi Groth16 EIP-2537:**
   - Jalankan `0x0c` MSM (19 public inputs + 1 basis = 20 titik $IC$) dan `0x0f` 4-pairing check. Revert jika invalid.

---

## 4. Analisis Upacara Setup Groth16 (Koreksi Klaim MPC Ceremony)

Penting untuk membedakan struktur upacara setup Groth16:
1. **Phase 1 (Powers of Tau):** Bersifat universal dan independen terhadap sirkuit. Parameter Phase 1 berukuran besar (misalnya $2^{20}$ s/d $2^{24}$ powers) dapat digunakan kembali dari upacara publik yang telah diaudit secara global (misal Hermez / Perpetual Powers of Tau).
2. **Phase 2 (Circuit-Specific Setup):** Bersifat terikat secara ketat pada sistem constraint R1CS sirkuit spesifik.
3. **Koreksi Terhadap Klaim "Single Ceremony":**
   - Mengadopsi JoinSplit sebagai satu-satunya sirkuit spend protokol berarti hanya diperlukan **satu kali upacara Phase 2 untuk modul spend saat pembekuan (circuit freeze)**.
   - Namun, hal ini **TIDAK** menjamin satu ceremony untuk seumur hidup protokol. Setiap modifikasi terhadap constraint, penambahan public input, atau penambahan sirkuit baru (misal sirkuit compliance masa depan) akan memerlukan upacara Phase 2 yang baru.

---

## 5. Roadmap & Integrasi Menuju Testnet Gate G

Sesuai arahan Founder (Zero Tech Debt):
1. **Fase 1 (DEC-035):** Selesaikan hardening sirkuit 1-in-1-out, Two-Limb quote decomposition, scope binding gadget, dan MMR indexer agar pipeline inti terbukti lolos E2E di Arbitrum Sepolia.
2. **Fase 2 (DEC-036):** Terapkan ekstensi JoinSplit 2-in-2-out secara terstruktur melintasi 5 layer setelah keputusan blocker desain diselesaikan oleh Founder dan sebelum upacara MPC Phase 2 digelar.

---

## 6. External References and Evidence

Dokumen ini disusun berlandaskan fakta teknis dan standar resmi berikut:
1. **Arbitrum Nitro ArbOS 40 Release:** [https://docs.arbitrum.io/run-a-node/arbos-releases/arbos40](https://docs.arbitrum.io/run-a-node/arbos-releases/arbos40)
   - *Bukti yang Didukung:* ArbOS 40 mengaktifkan dukungan Cancun / Dencun dan mengekspos precompiles EIP-2537 BLS12-381 pada environment Nitro & Stylus.
2. **Arbitrum Nitro ArbOS 51 Release:** [https://docs.arbitrum.io/run-a-node/arbos-releases/arbos51](https://docs.arbitrum.io/run-a-node/arbos-releases/arbos51)
   - *Bukti yang Didukung:* Pembaruan pemeliharaan dan kesiapan hardfork Prague / Pectra.
3. **EIP-2537 Precompile Specification:** [https://eips.ethereum.org/EIPS/eip-2537](https://eips.ethereum.org/EIPS/eip-2537)
   - *Bukti yang Didukung:* Alamat resmi `0x0c` untuk BLS12-381 G1 MSM dan `0x0f` untuk Pairing Check 4-pair.
4. **Zcash ZIP 315 (Best Practices for Wallet Implementations):** [https://zips.z.cash/zip-0315](https://zips.z.cash/zip-0315)
   - *Bukti yang Didukung:* Mengidentifikasi kebocoran metadata privasi akibat variasi jumlah input/output (*arity leakage*), flags change, dan log event pada layer transaksi.
5. **Circom Proving Circuits Documentation:** [https://docs.circom.io/getting-started/proving-circuits/](https://docs.circom.io/getting-started/proving-circuits/)
   - *Bukti yang Didukung:* Pemisahan tegas antara Powers of Tau (Phase 1 universal) dan circuit-specific compilation (Phase 2 spesifik).
6. **Filecoin Phase 2 Attestations Repository:** [https://github.com/filecoin-project/phase2-attestations](https://github.com/filecoin-project/phase2-attestations)
   - *Bukti yang Didukung:* Metodologi dan atestasi eksekusi Phase 2 ceremony untuk sirkuit Groth16 skala produksi.

---

## 7. Founder Executive Decisions on JoinSplit Blockers (Ratified)

Berdasarkan keputusan eksekutif Founder:

1. **Aktivasi Precompile Jaringan Target:**
   - **Keputusan:** Fokus 100% pada **Arbitrum Sepolia (`421614`)** untuk fase pra-testnet. Arbitrum One dipending hingga jadwal mainnet ditetapkan.
2. **Privasi Topologi Output (Direction B Diadopsi):**
   - **Keputusan:** Mengadopsi **Direction B (Strict Constant-Arity Topology)** secara permanen. Kontrak Stylus **selalu menyisipkan tepat 2 daun ke MMR** pada setiap transaksi spend (`_mmr_insert(cm1)` dan `_mmr_insert(cm2)`). Jika pengguna tidak membutuhkan kembalian kedua, sirkuit menghasilkan *Canonical Dummy Output Commitment* ($v=0$). Tambahan gas ~2.100 gas (<$0.0001 / Rp 0,8) disetujui demi mencapai privasi topologi 10/10 yang tidak bisa dianalisis pihak luar.
3. **Penanganan Fragmentasi Saldo Ekstrem ($N > 2$):**
   - **Keputusan:** Mengadopsi **In-Pool Progressive Consolidation** via `consolidate_notes` di SDK. Pengguna yang memiliki $\ge 3$ note kecil dapat menggabungkannya secara bertahap di dalam shielded pool hanya dengan membayar biaya gas relayer minimal, menjaga sirkuit tetap ramping (10.800 constraints) dan proving latency di bawah 2.5 detik.
4. **Privasi Sinkronisasi MMR (Paginated Bulk/Range Sync):**
   - **Keputusan:** Mengadopsi **Paginated Bulk/Range Sync**. Relayer melayani penarikan daun komitmen secara chunking (misal 1.000 daun per request = hanya ~32 KB). Dompet menyusun dan memverifikasi jalur MMR secara lokal (client-side), sehingga operator relayer sama sekali tidak dapat memetakan note mana yang dimiliki oleh pengguna.
