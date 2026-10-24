# Task Tracker: DEC-051 Nopenena Cryptographic Deniability, Small Decoy Sets & Dummy Note Architecture

Dokumen ini adalah tracker operasional implementasi untuk keputusan arsitektur resmi:
 **[`research/dec/DEC-051-nopenena-cryptographic-deniability-small-decoy-sets-dummy-notes.md`](research/dec/DEC-051-nopenena-cryptographic-deniability-small-decoy-sets-dummy-notes.md)**

---

##  Objektif & Target Teknis
Mengimplementasikan arsitektur **Cryptographic Deniability** berbasis riset Nopenena (IACR 2024/903) ke dalam Zeltra Protocol untuk memutus celah **analisis graf transaksi (DM-decomposition / change-note heuristic)**:
1. **R1CS Secret Permutation Gadget ($\pi \in \{0, 1\}$):** Menukar posisi Output 1 dan Output 2 secara rahasia di dalam sirkuit `JoinSplitCircuit` (+7 constraints).
2. **Companion Constraint Decoy Binding (DEC-046):** Mengunci kawat dummy auxiliary secara deterministik: $\text{is\_decoy} \cdot v_{\text{aux}} == 0$.
3. **Decoy Dummy Note Injection (`zeltra-sdk`):** Menghasilkan dummy note bernilai $0$ dengan komitmen Poseidon-W5 seragam dan dual 96B memo AEAD terenkripsi pada transaksi exact payment.
4. **Isolasi State di SDK 2PC Store:** Mengisolasi `NoteClassification::DecoyDummy` agar tidak tercampur dalam saldo aktif (`spendable_balance`).
5. **Verifikasi Jacobian Rigidity (DEC-047):** Mempertahankan target nullity finansial $r_{T_{\text{financial}}} = 0$ di seluruh 6 keluarga saksi adversarial.

---

##  Checklist Eksekusi Bertahap

### Fase 1: R1CS Circuit Hardening (`zeltra-core`)
- [x] **1.1 Konstruksi `DeniabilityPermutationGadget` di `zeltra-core/src/joinsplit_circuit.rs`:**
  - [x] Implementasikan alokasi saksi rahasia boolean $\pi \leftarrow \{0, 1\}$.
  - [x] Terapkan constraint kuadratik $\pi \cdot (1 - \pi) == 0$.
  - [x] Terapkan multiplexer bersyarat untuk nilai ($v_{\text{out}, 1}, v_{\text{out}, 2}$) dan komitmen ($C_{\text{out}, 1}, C_{\text{out}, 2}$).
  - [x] Terapkan companion constraint DEC-046: $\text{is\_decoy} \cdot v_{\text{aux}} == 0$.
- [x] **1.2 Integrasi ke `JoinSplitCircuit::generate_constraints`:**
  - [x] Sambungkan output target dan auxiliary ke `DeniabilityPermutationGadget`.
  - [x] Pastikan 19 Public Inputs tetap eksak tanpa pergeseran offset calldata.
- [x] **1.3 Integrasi ke Engine Jacobian (`zeltra-core/src/jacobian.rs` & `formal_tests.rs`):**
  - [x] Daftarkan kawat $\pi$ dan $\text{is\_decoy}$ ke pemetaan Jacobian diagnostics.
  - [x] Verifikasi matriks rank $M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$ membuktikan $r_T = 0$ pada FAM-1 s/d FAM-6.

---

### Fase 2: Client Wallet & State Management (`zeltra-sdk`)
- [x] **2.1 Model Klasifikasi Note di `zeltra-sdk/src/wallet/note_wallet.rs`:**
  - [x] Tambahkan enum variant `NoteClassification::DecoyDummy`.
  - [x] Pastikan fungsi `spendable_balance()` memfilter note berstatus `DecoyDummy`.
- [x] **2.2 Generator Decoy Dummy Note & Dual Memo Routing:**
  - [x] Implementasikan `generate_decoy_dummy_note(asset_id)` dengan $v = 0$, random $\rho$, random $r$.
  - [x] Update coin selection di `coin_selection.rs` agar otomatis menyuntikkan decoy dummy saat transaksi 1-in-1-out atau exact pay (2-in-1-out).
  - [x] Generate dual 96B memo AEAD terenkripsi menggunakan Ephemeral ECDH X25519 (DEC-048).
- [x] **2.3 Persistence & 2PC SQLite Isolation (`store.rs`):**
  - [x] Simpan tag decoy di database lokal SQLite/SQLCipher (`bundled-sqlcipher`).
  - [x] Tangani pemulihan note via viewing key tanpa memicu error desinkronisasi saldo.

---

### Fase 3: Rangkaian Pengujian (Test Ratio $\ge 2\times$ Negative Tests)

#### A. Positive Tests / Known-Answer Tests (KAT)
- [x] **KAT-1:** Happy path JoinSplit dengan $\pi = 0$ (Output 1 = Recipient, Output 2 = Change Note).
- [x] **KAT-2:** Happy path JoinSplit dengan $\pi = 1$ (Output 1 = Change Note, Output 2 = Recipient).
- [x] **KAT-3:** Exact payment dengan Decoy Dummy Injection (Output 1 = Recipient $v > 0$, Output 2 = Decoy $v = 0$).

#### B. Adversarial & Negative Boundary Tests (Minimal 6 Skenario Fail-Closed)
- [x] **NEG-1 (Tampered Non-Zero Decoy):** Saksi menyuntikkan $v_{\text{aux}} = 50$ dengan $\text{is\_decoy} = \text{true}$. Wajib revert pada companion constraint.
- [x] **NEG-2 (Non-Canonical Permutation Bit):** Saksi mencoba $\pi = 2$ atau $\pi = p - 1$. Wajib revert pada boolean gate $\pi(1-\pi) == 0$.
- [x] **NEG-3 (Decoy Counterfeit Inflow):** Penyerang mencoba membelanjakan dummy note ($v=0$) sebagai note bernilai $v > 0$ di transaksi berikutnya. Wajib ditolak oleh balance check.
- [x] **NEG-4 (Permutation Memo Mismatch):** Penyerang menukar urutan memo di calldata tanpa mencocokkan public inputs komitmen. Wajib ditolak verifier Stylus.
- [x] **NEG-5 (Field Overflow Boundary):** Nilai pembuta $r_{\text{dummy}} \ge r$. Wajib fail-closed di canonical scalar validator.
- [x] **NEG-6 (Double Decoy Spend Replay):** Mencoba membakar nullifier dummy note dua kali on-chain. Wajib revert `NullifierAlreadySpent`.

---

### Fase 4: Validasi Benchmark & Audit Kepatuhan
- [x] **4.1 Eksekusi Benchmark Empiris:**
  - [x] Jalankan `cargo run --release --bin bench_deniability`.
  - [x] Pastikan overhead constraint $\le 10$ gates (baseline data: +7 gates).
  - [x] Pastikan Shannon Entropy delta $\le 0.001$ bits.
- [x] **4.2 Code Quality & CI Hygiene:**
  - [x] `cargo fmt --check` rapi di seluruh workspace.
  - [x] `cargo clippy --all-targets -- -D warnings` bersih tanpa error.
  - [x] Semua unit test lolos 100%: `cargo test -p zeltra-core`.

---

##  Invariant Pengaman Mutlak (Non-Negotiable)
1. **Multi-Asset Solvency:** Total liabilities kontrak tidak boleh terpengaruh oleh dummy note bernilai $0$.
2. **Zero Degrees of Freedom (DEC-046):** Tidak ada kawat yang floating / unconstrained di slot decoy.
3. **Calldata Budget:** Tepat 19 Public Inputs dan dual 96-byte memo on-chain (`spend_joinsplit`).
4. **Fail-Closed State:** Kegagalan pembuktian ZK atau verifikasi tidak boleh memutasi state ledger.
