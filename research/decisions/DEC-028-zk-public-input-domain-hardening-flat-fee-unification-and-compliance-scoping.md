# DEC-028: Strict Relayer Preflight Ingress Hardening, Unified Flat 45 bps Protocol Outflow Fee, and ZK Statement Scoping

- **Status:** Proposed & Accepted (Architecture Core Decision)
- **Author:** Zeltra Protocol Architecture Team
- **Date:** 2026-10-07
- **Impact Areas:** `nimbus-node` (strict 12-input preflight, domain binding verification, quote simplification), `nimbus-contracts` (flat 45 bps fee unification, removal of root age branch), `nimbus-core` (pure flat fee policy), `docs/analisi`
- **Architectural Paradigm:** *"Fail-Closed at Ingress, Zero Domain Ambiguity, Pure Flat Predictability, Honest ZK Statements"*
- **Guiding Epistemological Principle:** *"A relayer must never broadcast a transaction whose validity it has not independently proven against its own execution domain, and a fee rail must never rely on unverified state timestamps."*

---

## 1. Konteks & Audit Panel Findings (Latar Belakang Review ZK Manual)

Pada audit konsistensi publik input ZK tanggal 7 Oktober 2026 ([`docs/analisi/zk-public-input-consistency-review.md`](file:///workspaces/Zeltra-Protocol/docs/analisi/zk-public-input-consistency-review.md)), ditemukan empat observasi kritis yang memerlukan tindakan arsitektur:

1. **R4 — Vektor Public Inputs Kosong Melewati Local Proof Preflight:**
   Di [`nimbus-node/src/handlers/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/handlers/spend.rs), blok validasi kanonikal, semantic binding, dan verifikasi lokal Groth16 (C-01 gas griefing defense) dibungkus oleh klausa `if !payload.public_inputs.is_empty()`. Jika pemanggil mengirim `public_inputs: []` atau tidak menyertakannya, seluruh preflight dilewati dan request langsung masuk ke antrian persistensi SQLite. Saat dispatcher mencoba membroadcast calldata transaksi ke Stylus, transaksi revert di on-chain dan relayer merugi akibat terbakarnya gas eksekusi.
2. **R3 — Domain Target Relayer (`chain_id` & `contract_address`) Tidak Divalidasi:**
   Handler memvalidasi kecocokan nilai untuk indeks 0–7, 10, dan 11, tetapi **mengabaikan indeks 8 (`chain_id`) dan 9 (`contract_address`)**. Akibatnya, proof yang dibuat untuk target jaringan lain (misal Ethereum L1 atau mock sandbox) akan lolos verifikasi lokal di node CPU relayer, tetapi pasti **revert** di smart contract Arbitrum Sepolia karena Stylus menyusun input dari runtime (`env_chain_id()` dan `env_contract_address()`).
3. **R2 — Umur Association Root Bukan Bukti Umur Kepemilikan Dana (Holding Fee Gaming):**
   Di jalur BLS legacy dan batch spend, fee dipotong dari 45 bps menjadi 40 bps jika parameter `root` memiliki umur registrasi $\ge 30$ hari di tabel `clean_association_roots`. Karena pemanggil bebas memilih root mana yang dikirim tanpa membuktikan kepemilikan note pada root tersebut, pengguna baru yang baru deposit 1 menit lalu dapat mencatut root tua untuk mendapatkan diskon 5 bps secara tidak sah (*fee tier arbitrage*).
4. **R1 — Compliance Circuit Belum Mengikat Association Membership:**
   Sirkuit prototype `ComplianceCircuit` mendeklarasikan 4 public inputs (`root`, `nullifier`, `recipient`, `amount`), namun hanya meng-constrain relasi `nullifier = Poseidon(secret, randomness)`. Input `root`, `recipient`, dan `amount` bersifat unconstrained di dalam sirkuit R1CS.

DEC-028 merumuskan resolusi terpadu untuk menyelesaikan keempat isu di atas secara tuntas, efisien, dan mathematically sound.

---

## 2. Keputusan Arsitektur 1: Pengerasan Ingress Relayer (Fix R4 & R3)

### A. Penegakan Wajib 12 Public Inputs (Fix R4 — Zero-Bypass Ingress Guard)
Relayer memberlakukan penegakan ketat di awal *endpoint admission*:
- Kolom `payload.public_inputs` **WAJIB** memiliki panjang tepat 12 elemen (`payload.public_inputs.len() == 12`).
- Jika panjangnya bukan 12 (termasuk jika kosong `[]` atau tidak disertakan), relayer langsung mengembalikan `HTTP 400 REJECTED` dengan pesan:
  `"Expected 12 public inputs for PrivateNoteCircuit, got N"`.
- Tidak ada transaksi spend ZK-UTXO yang dapat masuk antrian database (`enqueue_spend`) sebelum seluruh 12 scalar publik lolos validasi kanonikal, semantic binding, dan verifikasi Groth16 lokal.

### B. Validasi Domain Target Relayer (Fix R3 — Strict Domain Binding)
Relayer mengekstrak target eksekusi resminya dari runtime (`state.evm_client`):
- **Indeks 8 (`chain_id`):** Wajib sama persis dengan `U256::from(target_chain_id).to_be_bytes::<32>()`.
- **Indeks 9 (`contract_address`):** Wajib sama persis dengan alamat kontrak Stylus target yang dipad menjadi 32-byte big-endian (`Address -> [0u8; 32]`).
- Jika indeks 8 atau 9 tidak cocok, relayer menolak transaksi dengan:
  `"public_inputs[8] does not match target chain_id"` atau `"public_inputs[9] does not match target contract_address"`.

**Dampak Keamanan:** Menutup celah gas-griefing di mana relayer di-spam dengan proof lintas rantai yang lolos verifikasi lokal tetapi gagal di on-chain Stylus.

---

## 3. Keputusan Arsitektur 2: Penyeragaman Fee Flat 45 bps & Penghapusan Diskon 30 Hari (Fix R2)

### A. Mengapa Diskon Umur Root Dihapus Permanen
Memberikan diskon 5 bps (40 bps vs 45 bps) berdasarkan umur root terdaftar (`clean_association_roots`) dihapus secara total dari seluruh codebase karena:
1. **Mengeliminasi Celah Arbitrase Root (Anti-Gaming):** Tidak ada lagi vektor serangan di mana pengguna mengeksploitasi root historis untuk membayar fee lebih rendah.
2. **Efisiensi On-Chain & Penghematan Gas Stylus:** Smart contract tidak perlu lagi melakukan pembacaan storage (SLOAD) pada mapping `clean_association_roots` di dalam loop eksekusi `spend()` maupun `batch_spend()`.
3. **Penyederhanaan Node & Penghapusan Cache RPC:** Menghapus kebutuhan `root_timestamp_cache` di memori `AppState` dan menghilangkan query RPC histori ke blockchain saat menghitung quote fee.
4. **Keselarasan dengan Filosofi *"Volume Over TVL"*:** Zeltra adalah rel pembayaran dan settlement berkecepatan tinggi (seperti Stripe / Apple Pay), bukan money market yang mengunci modal diam. Fee protokol seragam **Flat 45 bps (0.45%)** untuk seluruh transaksi spend.

### B. Spesifikasi Kebijakan Fee Baru
| Jalur Transaksi | Nilai Biaya | Mekanisme Penegakan |
| :--- | :--- | :--- |
| **Deposit Inflow** | **0 bps (0.00% Permanen)** | Immutable Inflow Rail |
| **Private Spend Outflow (ZK-UTXO)** | **45 bps (0.45% Flat)** | Groth16 Value Conservation Constraint |
| **Legacy / Batch Spend (BLS)** | **45 bps (0.45% Flat)** | Reconstructed Fee Computation on Stylus |
| **Relayer Gas Execution** | **Gas Aktual + 15% Markup** | EIP-712 Quote Envelope Signed by User |

---

## 4. Keputusan Arsitektur 3: Penegasan Batas Statement Kriptografis (Fix R1)

Terkait temuan R1 pada sirkuit kepatuhan (`ComplianceCircuit`):
1. **Status Sirkuit:** `ComplianceCircuit` saat ini adalah sirkuit eksplorasi *proof of concept* preimage nullifier, dan **BUKAN** bukti kepemilikan dana pada Association Set (ASP).
2. **Isolasi Penuh dari Alur Pembayaran:** Smart contract Stylus menegaskan bahwa entry point view `verify_compliance` terisolasi 100% dan tidak pernah menjadi prasyarat transfer dana pada `spend_private_note()`.
3. **Roadmap Phase 3 (DEC-026 Alignment):** Bukti keanggotaan Association Set yang sesungguhnya akan diimplementasikan sebagai bagian dari ASP Receiver-Enforced Policy menggunakan gadget pohon Merkle penuh dengan bit-decomposition path verification. Tidak ada pihak yang boleh memasarkan `ComplianceCircuit` saat ini sebagai *Proof of Innocence* penuh sebelum upgrade Phase 3.

---

## 5. Matriks Dampak Implementasi & File Terkait

```text
┌─────────────────────────┬────────────────────────────────────────────────────────────────────────┐
│ Komponen                │ Tindakan Perubahan                                                     │
├─────────────────────────┼────────────────────────────────────────────────────────────────────────┤
│ nimbus-core/fees.rs     │ - Hapus `spend_fee_bps_for_holding` & `SPEND_FEE_30DAY_BPS`.           │
│                         │ - Satukan `private_spend_fee` menjadi flat 45 bps permanen.             │
│ nimbus-contracts/       │ - Hapus percabangan `delta_t >= 30 days` di `spend.rs` & `lib.rs`.     │
│                         │ - Setel `fee_bps = U256::from(45)` permanen (hemat SLOAD gas).          │
│ nimbus-node/spend.rs    │ - Hapus klausa bypass `if !public_inputs.is_empty()`.                  │
│                         │ - Wajibkan `public_inputs.len() == 12` (Fix R4).                       │
│                         │ - Validasi indeks 8 (chain_id) & indeks 9 (contract_addr) (Fix R3).   │
│ nimbus-node/quote.rs    │ - Hapus fungsi `resolve_fee_tier` & `get_root_timestamp`.              │
│                         │ - Hapus `root_timestamp_cache` dari `AppState`.                        │
│ docs/analisi/           │ - Catat status mitigasi R1, R2, R3, R4 sebagai SOLVED di DEC-028.      │
└─────────────────────────┴────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Referensi Literatur & Post-Mortem Keamanan Terkait

1. **Circomspect & Under-Constrained ZK Circuits Post-Mortem Analysis** — HashCloak, 0xPARC, Veridise (2023–2025). [Dasar teoritis pencegahan unconstrained signals pada Groth16 verification].
2. **ERC-4337 Bundler Simulation and Cross-Domain Replay Griefing Audits** — OpenZeppelin, Nethermind (2023–2025). [Dasar penegakan strict chain_id & contract_address binding pada relayer preflight].
3. **Euler Finance & zkLend Flash-Loan Utilization Curve Exploits** — BlockSec, Cyfrin, Sherlock (2023–2024). [Dasar penolakan parameter eksternal dinamis dalam penentuan tarif fee protokol].
4. **DEC-022: Defense Against Proof Settlement Mismatch Boundary Gaps** — Zeltra Protocol (2026).
5. **DEC-026: Receiver-Enforced Modular Compliance and Relayer Exposure Mitigation** — Zeltra Protocol (2026).
6. **DEC-027: Predictable Outflow Economic Model, Inflow Compliance Gate, and Relayer Anti-Limbo Mempool Lifecycle** — Zeltra Protocol (2026).
