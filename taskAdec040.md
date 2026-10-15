# TASK A (DEC-040A): Cryptographic Key Separation (sk/nk/fvk) & Groth16 Selective Disclosure Circuit (`zeltra-core`)

> **Scope:** `zeltra-core/` (Arkworks BLS12-381, Poseidon-W5, Selective Disclosure Circuit)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-040A-selective-disclosure-circuit-and-key-separation.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040A-selective-disclosure-circuit-and-key-separation.md) & [`research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)  
> **Prinsip:** *"Zero Spend Authority Leakage, Strict Nullifier Unforgeability, Non-Malleable Challenge Binding"*  
> **Status:** **PENDING IMPLEMENTATION**

---

## 1. Latar Belakang & Tujuan Arsitektur

Menyediakan fondasi kriptografi untuk kepatuhan institusi B2B dan pembuktian kepemilikan note ke CEX tanpa membocorkan spending key ($sk$):
1. **Pemisahan Kunci Tiga Serangkai (Zero Tech Debt):**
   Memisahkan secara kaku antara hak belanja ($sk$), hak pemantauan pembelanjaan on-chain ($nk$), dan hak dekripsi note ($ivk$/$ovk$). Full Viewing Key ($fvk$) memungkinkan audit penuh tanpa kuasa memindahkan dana.
2. **Sirkuit Pembuktian Selektif Groth16 (`SelectiveDisclosureCircuit`):**
   Sirkuit R1CS ringan (5 Public Inputs: `[owner_key, rho, nullifier, challenge_hi, challenge_lo]`, ~450 constraints, waktu pembuktian $< 15\text{ ms}$) untuk membuktikan keaslian nullifier dan kepemilikan note secara matematis.
3. **Pengikatan Non-Malleable Challenge Two-Limb:**
   Mengikat digest 256-bit verifier challenge ke dalam dua elemen skalar 128-bit (`challenge_hi`, `challenge_lo`) sesuai standar DEC-035A untuk mencegah *Stolen Proof Attack*.

---

## 2. Checklist Implementasi Rinci

### A. Hierarki & Pemisahan Kunci (`zeltra-core/src/note.rs`)
- [ ] **Struktur dan Implementasi `FullViewingKey`:**
  - Definisikan `FullViewingKey`:
    ```rust
    #[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
    pub struct FullViewingKey {
        pub owner_key: Fr,
        pub incoming_viewing_key: [u8; 32],
        pub outcoming_viewing_key: [u8; 32],
        pub nullifier_deriving_key: Fr,
    }
    ```
  - Implementasikan trait `Zeroize` dan `Drop` untuk `FullViewingKey` guna membersihkan buffer memori `incoming_viewing_key` dan `outcoming_viewing_key`.
  - Implementasikan fungsi derivasi:
    `pub fn derive_nullifier_key(spending_key: Fr) -> Fr` menggunakan `poseidon_w3(spending_key, 0, DOMAIN_NULLIFIER_KEY)`.
  - Tambahkan helper method pada `FullViewingKey`:
    - `pub fn derive_nullifier(&self, commitment: Fr, leaf_index: u64, epoch_id: u32) -> Fr`
    - `pub fn derive_joinsplit_nullifier(&self, rho: Fr, epoch_id: u64) -> Fr`
  - Implementasikan konversi hex kanonikal `to_hex(&self) -> String` dan `from_hex(hex_str: &str) -> Result<Self, KeyFormatError>`.

### B. Sirkuit R1CS `SelectiveDisclosureCircuit` (`zeltra-core/src/selective_disclosure_circuit.rs`)
- [ ] **Definisi Struct & Witness:**
  ```rust
  pub struct SelectiveDisclosureCircuit {
      // Private Witness
      pub spending_key: Option<Fr>,
      pub epoch_id: Option<u32>,
      // Public Inputs (5-PI)
      pub owner_key: Option<Fr>,
      pub rho: Option<Fr>,
      pub nullifier: Option<Fr>,
      pub challenge_hi: Option<Fr>,
      pub challenge_lo: Option<Fr>,
  }
  ```
- [ ] **Implementasi `ConstraintSynthesizer<Fr>`:**
  - **Tahap 1: Alokasi Public Inputs:** Alokasikan tepat 5 public input variables secara berurutan: `owner_key_var` (0), `rho_var` (1), `nullifier_var` (2), `challenge_hi_var` (3), `challenge_lo_var` (4).
  - **Tahap 2: Alokasi Private Witness:** Alokasikan saksi privat `spending_key_var` dan `epoch_id_var`.
  - **Tahap 3: Derivasi Nullifier Key Gadget:**
    Hitung $nk\_var = \text{Poseidon\_W3}(spending\_key\_var, 0, \text{DOMAIN\_NULLIFIER\_KEY})$.
  - **Tahap 4: Derivasi Nullifier Kanonikal Gadget:**
    Hitung $computed\_nf = \text{Poseidon\_W5}([nk\_var, rho\_var, epoch\_id\_var, 0]; \text{DOMAIN\_NULLIFIER})$.
    Tegakkan constraint persamaan: $computed\_nf.enforce\_equal(\&nullifier\_var)?$.
  - **Tahap 5: Formal Scope Binding Gadget (DEC-035B Compliant):**
    Ikat `challenge_hi_var` dan `challenge_lo_var` ke dalam constraint linear aktif agar tidak dieliminasi oleh optimizer R1CS.

### C. Prover & Verifier Helper Functions (`zeltra-core/src/selective_disclosure_circuit.rs`)
- [ ] `pub fn generate_disclosure_proof(circuit: SelectiveDisclosureCircuit, pk: &ProvingKey<Bls12_381>) -> Result<Proof<Bls12_381>, SynthesisError>`
- [ ] `pub fn verify_disclosure_proof(proof: &Proof<Bls12_381>, public_inputs: &[Fr], vk: &PreparedVerifyingKey<Bls12_381>) -> Result<bool, SynthesisError>`
- [ ] Buat fungsi pembuat parameter setup sirkuit untuk testing: `pub fn setup_disclosure_circuit_params<R: RngCore>(rng: &mut R) -> (ProvingKey<Bls12_381>, VerifyingKey<Bls12_381>)`.

### D. Re-ekspor & Modul (`zeltra-core/src/lib.rs`)
- [ ] Tambahkan `pub mod selective_disclosure_circuit;` ke `zeltra-core/src/lib.rs`.
- [ ] Re-ekspor: `FullViewingKey`, `SelectiveDisclosureCircuit`, `generate_disclosure_proof`, `verify_disclosure_proof`.

---

## 3. Matriks Pengujian Unit & Formal (`zeltra-core/tests/`)

### Positive Tests:
- [ ] `test_selective_disclosure_circuit_roundtrip`: Setup dummy params, buat proof dengan valid witness, verifikasi Groth16 sukses (return `true`).
- [ ] `test_full_viewing_key_nullifier_derivation_parity`: Nullifier yang dihitung dari `fvk.nullifier_deriving_key` sama persis dengan nullifier yang dihitung dari spending key asli.
- [ ] `test_fvk_hex_serialization_roundtrip`: Enkoding dan dekoding FVK hex menghasilkan nilai yang identik.

### Negative Tests (Security & Circuit Soundness):
- [ ] `test_disclosure_circuit_rejects_spoofed_nullifier`: Mencoba membuktikan nullifier orang lain menghasilkan constraint dissatisfaction / verification failed.
- [ ] `test_disclosure_circuit_rejects_tampered_challenge`: Mengubah `challenge_hi` atau `challenge_lo` saat verifikasi menghasilkan invalid proof.
- [ ] `test_disclosure_circuit_rejects_invalid_sk`: Memasukkan $sk$ yang salah gagal menghasilkan proof yang cocok dengan nullifier / owner_key.
- [ ] `test_fvk_zeroize_on_drop`: Memverifikasi byte rahasia IVK/OVK di dalam FVK dibersihkan dengan nol saat drop.

---

## 4. Strict Engineering Guardrails (Zero-Slop Policy)

1. **NO PLACEHOLDERS:** Dilarang menggunakan `todo!()`, `unimplemented!()`, dummy byte arrays, atau mock proofs di jalur production.
2. **COMPILE-CLEAN:** Wajib lulus `cargo check -p zeltra-core` dan `cargo test -p zeltra-core` dengan 0 error dan 0 warning.
3. **FORMAT & CLIPPY:** Wajib bersih `cargo fmt --check` dan `cargo clippy -p zeltra-core -- -D warnings`.
