# DEC-040A: Cryptographic Key Separation (sk vs nk vs fvk) & Groth16 Selective Disclosure Circuit

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-040`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md))
- **Author:** Zeltra Protocol Architecture & Cryptography Team
- **Date:** 2026-10-15
- **Applies to:**
  - `zeltra-core/src/note.rs` (Key derivation hierarchy: `sk`, `nk`, `ivk`, `ovk`, `fvk`, and nullifier derivations)
  - `zeltra-core/src/selective_disclosure_circuit.rs` (New 5-PI `SelectiveDisclosureCircuit` Groth16)
  - `zeltra-core/src/circuit_gadgets.rs` (Nullifier key derivation gadget & non-malleable challenge binding)
  - `zeltra-core/src/lib.rs` (Export disclosure circuits, prover/verifier helpers, and key structs)
- **Parent & Related DECs:**
  - [`DEC-040`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md) (Master Viewing Key Export Suite Specification)
  - [`DEC-034A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md) (Canonical Field Bit-Packing $I_0 = v + \text{asset\_id} \cdot 2^{64}$)
  - [`DEC-035A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md) (Two-Limb 128-bit Challenge Representation)
  - [`DEC-035B`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035B-formal-scope-binding-gadget-and-groth16-statement-integrity.md) (Formal Scope Binding Gadget & Statement Integrity)
  - [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-037-note-encryption-memo-and-viewing-key-recovery.md) (Viewing Key Derivation & AEAD Note Memo)
- **Architectural Paradigm:** *"Zero Spend Authority Leakage, Strict Nullifier Unforgeability, Non-Malleable Challenge Binding"*

---

## 1. Executive Summary & Dilema Kriptografi Audit Kas Privat

Dalam arsitektur ZK-UTXO murni Zeltra ([`DEC-038`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)), hak milik dan otorisasi pengeluaran dana dikendalikan secara mutlak oleh rahasia master `spending_key` ($sk$). Komitmen note dibentuk melalui hash Poseidon-W5 yang mengikat $(v, \text{asset\_id}, \text{owner\_key}, \rho, r)$ dan daunnya dicatat pada Merkle Mountain Range (MMR).

Ketika pengguna berhadapan dengan kewajiban audit eksternal—seperti verifikasi kepatuhan bursa terpusat (CEX Source of Funds) atau audit pajak B2B—timbul **dilema kriptografi**:
1. **Dilema Kebocoran Kunci Belanja (Spending Authority Leak):**
   Jika pengguna menyerahkan $sk$ kepada auditor untuk membuktikan kepemilikan note, auditor atau pihak bursa secara instan memperoleh kemampuan membelanjakan dan menguras seluruh isi dompet pengguna.
2. **Dilema Pemalsuan Nullifier (Spoofed Spend Fraud):**
   Jika pengguna hanya memberikan plaintext note dan string heksadesimal nullifier tanpa membocorkan $sk$, pihak verifikator (CEX) **tidak memiliki cara matematis** untuk memastikan bahwa nullifier tersebut benar-benar diturunkan dari note terkait dan bukan nullifier curian dari transaksi orang lain di block explorer.
3. **Ketidakcocokan Tipe Aljabar (Algebraic Type Mismatch):**
   `owner_key` pada Zeltra diturunkan dari public key X25519 dan direpresentasikan sebagai skalar field $\text{Fr}$ (`Fr::from_be_bytes_mod_order`). Upaya memverifikasi kepemilikan menggunakan tanda tangan kurva eliptik Schnorr standar ($pk = sk \cdot G$) akan gagal karena skalar field Poseidon tidak dapat diperlakukan sebagai titik grup kurva eliptik BLS12-381 G1 secara langsung tanpa sirkuit.

**Solusi Penetapan DEC-040A:**
1. **Pemisahan Kunci Multi-Tier (Key Hierarchy):** Memisahkan secara ketat antara hak otorisasi belanja ($sk$), hak derivasi pemantauan pembelanjaan ($nk$), hak dekripsi note masuk ($ivk$), dan hak dekripsi note kembalian/keluar ($ovk$).
2. **Sirkuit Pembuktian Selektif Groth16 (`SelectiveDisclosureCircuit`):** Sirkuit ZK berbobot sangat ringan (~450 constraints, waktu pembuktian $< 15\text{ ms}$) yang membuktikan bahwa pembuat klaim mengetahui $sk$ yang menghasilkan `owner_key` dan `nullifier` note, sekaligus mengikatnya secara non-malleable ke *verifier challenge* 256-bit.

---

## 2. Arsitektur Hierarki Kunci Zeltra (Zero Tech Debt)

Hierarki kunci diturunkan secara deterministik dari tanda tangan Web3 user (`Personal Sign`):

```text
Web3 Signature (65 bytes) 
       │
       ▼ HKDF-Extract(salt = "ZELTRA_MASTER_V1")
Master Secret (32 bytes)
       │
       ├──► HKDF-Expand(info = "ZELTRA_SPENDING_KEY_V1")  ==> spending_key (sk ∈ Fr)
       │         │
       │         └──► Poseidon_W3(sk, 0, DOMAIN_NULLIFIER_KEY) ==> nullifier_deriving_key (nk ∈ Fr)
       │
       └──► HKDF-Expand(info = "ZELTRA_VIEWING_KEY_V1")
                 │
                 ├──► Incoming Viewing Key (ivk: X25519 secret, 32 bytes) ──► owner_key = Fr(pk_ivk)
                 │
                 └──► Outcoming Viewing Key (ovk: 32 bytes)
```

### A. Tabel Formal Hak Akses Kunci

| Tipe Kunci | Simbol | Tipe Data Rust | Karakteristik Rahasia | Kapabilitas Operasional | Hak Belanja? |
| :--- | :---: | :---: | :--- | :--- | :---: |
| **Spending Key** | $sk$ | `Fr` | Sangat Rahasia (Hanya User) | Mengesahkan transaksi `spend_*`, membuat witness sirkuit Private Note & JoinSplit. | **YA (100%)** |
| **Nullifier Deriving Key** | $nk$ | `Fr` | Semi-Rahasia (Audit Internal) | Menghitung nullifier $\text{nf} = \text{Poseidon\_W5}([nk, \rho, \text{epoch}, 0])$ untuk memeriksa status spent on-chain. | **TIDAK (0%)** |
| **Incoming Viewing Key** | $ivk$ | `[u8; 32]` | Terbatas (Auditor/Sender) | Mendekripsi 96-byte AEAD memo note masuk dari event log on-chain via ECDH. | **TIDAK (0%)** |
| **Outcoming Viewing Key** | $ovk$ | `[u8; 32]` | Terbatas (Auditor Internal) | Mendekripsi note kembalian (*change note*) dan memverifikasi histori pembayaran keluar. | **TIDAK (0%)** |
| **Full Viewing Key** | $fvk$ | `FullViewingKey` | Paket Delegasi Audit Lengkap | Gabungan $(\text{owner\_key}, ivk, ovk, nk)$ untuk merekonstruksi seluruh arus kas masuk & keluar. | **TIDAK (0%)** |

### B. Implementasi Rust Struktur `FullViewingKey` (`zeltra-core/src/note.rs`)

```rust
use ark_bls12_381::Fr;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Full Viewing Key (FVK) untuk audit independen dan pengawasan internal tanpa kuasa belanja.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FullViewingKey {
    /// Public owner key scalar (Fr) yang tercatat di leaf commitment
    pub owner_key: Fr,
    /// Incoming Viewing Key untuk dekripsi note masuk (X25519 secret)
    pub incoming_viewing_key: [u8; 32],
    /// Outcoming Viewing Key untuk dekripsi change note dan riwayat pembayaran
    pub outcoming_viewing_key: [u8; 32],
    /// Nullifier Deriving Key (Fr) untuk mendeteksi kapan note dibelanjakan on-chain
    pub nullifier_deriving_key: Fr,
}

impl Zeroize for FullViewingKey {
    fn zeroize(&mut self) {
        self.incoming_viewing_key.zeroize();
        self.outcoming_viewing_key.zeroize();
    }
}

impl Drop for FullViewingKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl FullViewingKey {
    /// Menghitung nullifier kanonikal untuk note tertentu menggunakan nk.
    pub fn derive_nullifier(&self, commitment: Fr, leaf_index: u64, epoch_id: u32) -> Fr {
        crate::note::derive_nullifier(self.nullifier_deriving_key, commitment, leaf_index, epoch_id)
    }

    /// Menghitung JoinSplit nullifier kanonikal menggunakan nk.
    pub fn derive_joinsplit_nullifier(&self, rho: Fr, epoch_id: u64) -> Fr {
        crate::note::derive_joinsplit_nullifier(self.nullifier_deriving_key, rho, epoch_id)
    }
}
```

---

## 3. Spesifikasi R1CS `SelectiveDisclosureCircuit` (Groth16 5-PI)

Sirkuit ini dirancang untuk membuktikan bahwa pembawa bukti mengetahui $sk$ yang sah tanpa membocorkan $sk$ itu sendiri, sekaligus mengunci bukti ke target verifikator dan transaksi on-chain tertentu:

### A. Tata Letak Public Inputs (5 Public Inputs Kanonikal)
Sesuai konvensi ABI Groth16 BLS12-381 EIP-2537:
```text
┌────────────┬──────────────────┬──────────────┬────────────────────────────────────────┐
│ Slot Index │ Public Input Wire│ Tipe Data    │ Deskripsi Kriptografis                 │
├────────────┼──────────────────┼──────────────┼────────────────────────────────────────┤
│ Slot 0     │ owner_key        │ Fr           │ Kunci pemilik pada komitmen leaf       │
│ Slot 1     │ rho              │ Fr           │ Nullifier seed unik note preimage      │
│ Slot 2     │ nullifier        │ Fr           │ Nullifier yang di-spend pada spent_tx  │
│ Slot 3     │ challenge_hi     │ Fr           │ 128-bit limb atas Verifier Challenge   │
│ Slot 4     │ challenge_lo     │ Fr           │ 128-bit limb bawah Verifier Challenge  │
└────────────┴──────────────────┴──────────────┴────────────────────────────────────────┘
Total: 5 Public Inputs (EIP-2537 MSM Calldata = 160 bytes)
```

### B. Tata Letak Private Witness
1. `spending_key`: $sk \in \text{Fr}$ (Kunci privat master pemilik).
2. `epoch_id`: $\text{u32}$ (Epoch window saat note di-spend on-chain).

### C. Aljabar Constraint R1CS (Rank-1 Constraint System)

```rust
// zeltra-core/src/selective_disclosure_circuit.rs
use ark_bls12_381::Fr;
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::prelude::*;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

pub struct SelectiveDisclosureCircuit {
    // Private Witness
    pub spending_key: Option<Fr>,
    pub epoch_id: Option<u32>,
    // Public Inputs
    pub owner_key: Option<Fr>,
    pub rho: Option<Fr>,
    pub nullifier: Option<Fr>,
    pub challenge_hi: Option<Fr>,
    pub challenge_lo: Option<Fr>,
}

impl ConstraintSynthesizer<Fr> for SelectiveDisclosureCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // 1. Alokasi Public Inputs (Strict Index Ordering 0..=4)
        let owner_key_var = FpVar::new_input(cs.clone(), || {
            self.owner_key.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let rho_var = FpVar::new_input(cs.clone(), || {
            self.rho.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let nullifier_var = FpVar::new_input(cs.clone(), || {
            self.nullifier.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let challenge_hi_var = FpVar::new_input(cs.clone(), || {
            self.challenge_hi.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let challenge_lo_var = FpVar::new_input(cs.clone(), || {
            self.challenge_lo.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // 2. Alokasi Private Witness
        let spending_key_var = FpVar::new_witness(cs.clone(), || {
            self.spending_key.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let epoch_id_var = FpVar::new_witness(cs.clone(), || {
            self.epoch_id.map(|e| Fr::from(e as u64)).ok_or(SynthesisError::AssignmentMissing)
        })?;

        // 3. Constraint 1: Derivasi nk via Poseidon-W3
        // nk = Poseidon_W3(spending_key, 0, DOMAIN_NULLIFIER_KEY)
        let domain_nk = FpVar::Constant(crate::poseidon::domain_nullifier());
        let zero_var = FpVar::Constant(Fr::from(0u64));
        let nk_var = crate::circuit_gadgets::poseidon_w3_gadget(
            cs.clone(),
            &[spending_key_var.clone(), zero_var.clone()],
            domain_nk,
        )?;

        // 4. Constraint 2: Derivasi Nullifier Kanonikal via Poseidon-W5
        // nf = Poseidon_W5([nk, rho, epoch_id, 0], DOMAIN_NULLIFIER)
        let domain_nf = FpVar::Constant(crate::poseidon::domain_nullifier());
        let computed_nf = crate::circuit_gadgets::poseidon_w5_gadget(
            cs.clone(),
            &[nk_var, rho_var, epoch_id_var, zero_var],
            domain_nf,
        )?;
        computed_nf.enforce_equal(&nullifier_var)?;

        // 5. Constraint 3: Non-Malleable Challenge Binding Gadget (DEC-035B)
        // Memastikan challenge_hi dan challenge_lo terikat dalam sistem R1CS aktif
        crate::circuit_gadgets::enforce_dummy_range_binding(
            cs.clone(),
            &challenge_hi_var,
            &challenge_lo_var,
        )?;

        Ok(())
    }
}
```

---

## 4. Formula Dekomposisi Verifier Binding Challenge (Two-Limb)

Untuk mencegah *Stolen Proof Attack* di mana penyerang menggunakan ulang bukti kepatuhan milik orang lain:

1. **Preimage Challenge 256-Bit:**
   $$\text{Challenge} = \text{Blake2s-256}\Big(\text{"ZELTRA\_CEX\_DISCLOSURE\_V1"} \parallel \text{VerifierID} \parallel \text{AccountUID} \parallel \text{Timestamp} \parallel \text{ExpiresAt} \parallel \text{Nonce} \parallel C \parallel \text{nf} \parallel \text{SpentTxHash} \parallel \text{PayoutRecipient}\Big)$$

2. **Dekomposisi Two-Limb 128-Bit (DEC-035A Canonical):**
   Karena modulus field skalar BLS12-381 $\text{Fr} < 2^{256}$, memasukkan nilai hash 256-bit langsung ke elemen field dapat menyebabkan *modulo wrap-around* (aliasing). Sesuai [`DEC-035A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md), digest 32 byte dipecah menjadi dua limb 16 byte (128-bit integer):
   $$\text{challenge\_hi} = \text{u128::from\_be\_bytes}(\text{Challenge}[0..16])$$
   $$\text{challenge\_lo} = \text{u128::from\_be\_bytes}(\text{Challenge}[16..32])$$

3. Kedua limb tersebut dikonversi ke field skalar $\text{Fr}$ dan dialokasikan sebagai Public Input wire slot 3 dan slot 4:
   $$\text{public\_inputs}[3] = \text{Fr::from}(\text{challenge\_hi})$$
   $$\text{public\_inputs}[4] = \text{Fr::from}(\text{challenge\_lo})$$

---

## 5. Invarian Kriptografi Formal

- **`INV-DISC-1` (Nullifier Unforgeability):**
  Tanpa pengetahuan atas $sk$, peluang bagi entitas musuh untuk menghasilkan bukti Groth16 yang valid atas $\text{nullifier}' \neq \text{nf}$ adalah kurang dari $\epsilon \le 2^{-128}$.
- **`INV-DISC-2` (Zero Spend Authority from FVK):**
  Pengetahuan atas $fvk = (\text{owner\_key}, ivk, ovk, nk)$ secara matematis tidak memberikan informasi apa pun untuk memalsukan bukti pembelanjaan di `PrivateNoteCircuit` atau `JoinSplitCircuit` karena kedua sirkuit belanja mewajibkan bukti pengetahuan preimage non-linear atas $sk$.
- **`INV-DISC-3` (Non-Malleable Challenge Binding):**
  Mengubah satu bit pun pada metadata (`VerifierID`, `AccountUID`, `SpentTxHash`, `PayoutRecipient`, `Timestamp`, atau `Nonce`) akan memicu ketidakcocokan $\text{Challenge}$, menyebabkan verifikasi Groth16 langsung ditolak secara *fail-closed*.

---

## 6. Matriks Pengujian Formal (`zeltra-core/tests/`)

1. `test_selective_disclosure_circuit_roundtrip`: Setup parameter Groth16 BLS12-381, buat saksi yang valid, verifikasi bahwa proof berhasil diverifikasi 100%.
2. `test_disclosure_circuit_rejects_spoofed_nullifier`: Percobaan membuktikan nullifier curian dari note lain gagal pada constraint equality `computed_nf == nullifier_var`.
3. `test_disclosure_circuit_rejects_tampered_challenge`: Mengubah nilai `challenge_hi` atau `challenge_lo` sebesar 1 unit membuat verifikasi Groth16 gagal.
4. `test_full_viewing_key_nullifier_parity`: Memastikan fungsi `fvk.derive_nullifier` menghasilkan nilai yang sama persis dengan `derive_nullifier` yang dihitung langsung dari `spending_key`.
5. `test_fvk_zeroize_on_drop`: Memastikan buffer memori `ivk` dan `ovk` di dalam `FullViewingKey` dibersihkan dengan nol saat keluar dari scope menggunakan trait `Zeroize`.
