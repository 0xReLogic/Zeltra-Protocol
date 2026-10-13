# DEC-034A: Canonical Field Bit-Packing, Intra-Asset Circuit Soundness, and DEC-037 Plaintext Extension

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-034`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md))
- **Author:** Zeltra Protocol Architecture & Cryptography Team
- **Date:** 2026-10-13
- **Applies to:**
  - `zeltra-core/src/note.rs` (`value_packed` Poseidon leaf commitment, 48-byte `NotePlaintext` byte-44 `asset_id` mapping)
  - `zeltra-core/src/note_circuit.rs` (`PrivateNoteCircuit` 16-PI `flags_packed` asset constraint, 64-bit range proof)
  - `zeltra-core/src/joinsplit_circuit.rs` (`JoinSplitCircuit` 19-PI `flags_packed` asset equality & constant topology)
  - `zeltra-core/src/fees.rs` (Dual-asset fee calculations, Gwei scale precision)
- **Parent & Related DECs:**
  - [`DEC-034`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md) (Master Dual-Asset Architecture Blueprint)
  - [`DEC-034B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034B-stylus-siloed-solvency-payable-weth-wrap-and-multi-asset-settlement.md) (Stylus Smart Contract Siloed Solvency & Payable Auto-Wrap)
  - [`DEC-034C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034C-relayer-zero-oracle-reimbursement-and-sdk-multi-asset-wallet.md) (Relayer Zero-Oracle Routing & Multi-Asset Client SDK)
  - [`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-formal-specification-bls12-381-poseidon-and-groth16.md) (Audited Grain-128 LFSR Poseidon-W5 Constants)
  - [`DEC-035A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md) (16-PI Public Input Layout & Two-Limb Quote Hash)
  - [`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md) (Universal 2-in-2-out JoinSplit 19-PI Circuit Soundness)
  - [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-037-note-encryption-memo-and-viewing-key-recovery.md) (96-Byte AEAD Memo & 48-Byte Note Plaintext)
- **Architectural Paradigm:** *"Zero-Overhead Field Bit-Packing, Constant-Arity Width-5 Preservation, Strict Intra-Asset Value Conservation"*

---

## 1. Executive Summary & Dilema Kriptografi State Width

Pada saat merancang dukungan dual-asset (WETH dan USDC) ke dalam arsitektur Zeltra, tim kriptografi dihadapkan pada dilema keterbatasan arity hash Poseidon:

1. **State Width-5 Limitation:** Implementasi hash Poseidon di `zeltra-core` ([`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-formal-specification-bls12-381-poseidon-and-groth16.md)) dikonfigurasi dengan lebar state $t = 5$:
   $$\text{State} = [D, I_0, I_1, I_2, I_3]$$
   Di mana $D$ adalah domain separator (`domain_note_commitment`), dan $I_0 \dots I_3$ adalah 4 elemen payload lapangan: `value`, `owner_key`, `rho`, dan `randomness`.
2. **Bahaya Ekspansi ke Width-6 (Over-Arity Trap):**
   Jika `asset_id` ditambahkan sebagai elemen input ke-5, state Poseidon harus diperlebar menjadi $t = 6$. Konsekuensinya fatal:
   - Membutuhkan regenerasi round constants baru via Grain-128 LFSR.
   - Menaikkan jumlah non-linear S-box constraints dalam R1CS sirkuit sebesar $\sim 25\%$.
   - Membatalkan seluruh parameter precomputed constants yang telah diuji di `zeltra-contracts/src/poseidon_w5_constants.rs`.

**Solusi Penetapan DEC-034A:** Menerapkan **Canonical Field Bit-Packing** di dalam elemen $I_0$. Karena rentang `value` dijamin masuk dalam integer 64-bit ($0 \le v < 2^{64}$), dan field scalar BLS12-381 $\text{Fr}$ memiliki kapasitas $\approx 254.7$ bit, kita dapat mengemas `value` dan `asset_id` ke dalam satu field skalar $\text{Fr}$ tunggal tanpa risiko overflow modulus field.

---

## 2. Spesifikasi Matematika Bit-Packing

### A. Formula Aljabar di Lapangan Skalar $\text{Fr}$

Definisikan identifier aset resmi:
$$\text{Asset ID} \in \{0, 1\} \quad (0: \text{WETH}, \ 1: \text{USDC})$$

Elemen input pertama komitmen leaf dirumuskan sebagai:
$$\text{value\_packed} = \text{Fr::from}(value) + \text{Fr::from}(asset\_id) \cdot 2^{64}$$

Di mana $2^{64}$ di lapangan $\text{Fr}$ BLS12-381 adalah konstanta kanonikal:
$$2^{64} = 18{,}446{,}744{,}073{,}709{,}551{,}616 \pmod r$$

### B. Bukti Non-Aliasing & Pemisahan Domain Bit
Modulus lapangan skalar BLS12-381 $r$ adalah:
$$r = 52435875175126190479447740508185965837690552500527637822603658699938581184513 \approx 5.24 \times 10^{76}$$

Nilai maksimum yang mungkin dihasilkan oleh formula bit-packing:
$$\text{value\_packed}_{\max} = (2^{64} - 1) + 1 \cdot 2^{64} = 2^{65} - 1 \approx 3.68 \times 10^{19}$$

Karena:
$$2^{65} - 1 \ll r$$
Maka operasi penjumlahan dan perkalian skalar dijamin **tidak mengalami reduksi modular (wrap-around / aliasing)** di lapangan $\text{Fr}$. Nilai `value` dan `asset_id` dapat direkonstruksi dan diisolasi secara deterministik tanpa ambiguitas.

---

## 3. R1CS Circuit Gadget & Constraint Enforcement

Di dalam sirkuit ZK (`PrivateNoteCircuit` dan `JoinSplitCircuit`), relasi bit-packing ditegakkan secara formal:

```rust
// zeltra-core/src/circuit_gadgets.rs (Bit-Packing Gadget)
pub fn enforce_packed_value_and_asset(
    cs: &ConstraintSystemRef<Fr>,
    value_packed_var: &FpVar<Fr>,
    value_var: &FpVar<Fr>,
    asset_id_var: &FpVar<Fr>,
    value_witness: Option<u64>,
    asset_id_witness: Option<u8>,
) -> Result<(), SynthesisError> {
    use ark_r1cs_std::eq::EqGadget;

    let zero = FpVar::Constant(Fr::from(0u64));
    let one = FpVar::Constant(Fr::from(1u64));

    // 1. Boolean check pada asset_id: asset_id * (1 - asset_id) == 0
    let one_minus_asset = &one - asset_id_var;
    let bool_check = asset_id_var * &one_minus_asset;
    bool_check.enforce_equal(&zero)?;

    // 2. Strict 64-bit integer range decomposition pada value
    enforce_u64_range(cs, value_var, value_witness.map(Fr::from))?;

    // 3. Linear combination constraint:
    // value_packed == value + asset_id * 2^64
    let two_power_64 = FpVar::Constant(Fr::from_u128(1u128 << 64));
    let reconstructed = value_var + (asset_id_var * &two_power_64);
    reconstructed.enforce_equal(value_packed_var)?;

    Ok(())
}
```

### Jumlah R1CS Constraints:
* Boolean check `asset_id`: 1 constraint kuadratik.
* Range check 64-bit `value`: 64 boolean constraints + 1 linear combination = 65 constraints.
* Rekonstruksi linear: 1 constraint.
* **Total Overhead:** Tepat **67 constraints R1CS per note**, sangat ringan dan tidak berdampak signifikan terhadap proving time ($< 0.5\%$).

---

## 4. Preservasi Public Inputs (16-PI & 19-PI Preserved)

Untuk menghindari penambahan biaya gas pada verifikasi precompile EIP-2537 (`0x0c` MSM + `0x0f` Pairing) di Stylus, `asset_id` tidak dialokasikan sebagai Public Input mandiri baru. Sebaliknya, `asset_id` dikemas ke dalam bit field `flags_packed`:

### A. Pada `PrivateNoteCircuit` (DEC-035A, 16 Public Inputs)
Slot ke-15 (`flags_packed`, menggantikan boolean `has_change` dan `is_rollover` sederhana):
* Bit 0: `has_change` (0 atau 1)
* Bit 1: `is_rollover` (0 atau 1)
* Bit 2..=9: `asset_id` (0: WETH, 1: USDC)

### B. Pada `JoinSplitCircuit` (DEC-036A, 19 Public Inputs)
Slot ke-17 (`flags_packed`):
* Bit 0: `has_change_1`
* Bit 1: `has_change_2`
* Bit 2: `is_rollover`
* Bit 3..=10: `asset_id`

### C. Konservasi Nilai Intra-Aset (Anti-Penumbra Exploit 2024–2025)
Sirkuit menegakkan kesetaraan aset mutlak antara seluruh input dan output:
$$\text{asset\_id}_{in1} == \text{asset\_id}_{in2} == \text{asset\_id}_{out1} == \text{asset\_id}_{out2} == \text{public\_asset\_id}$$

Jika penyerang mencoba memasukkan note WETH dan mengklaim payout USDC (cross-asset mismatch), constraint kesetaraan akan langsung gagal, mencegah eksploitasi pencetakan saldo unbacked.

---

## 5. Ekstensi DEC-037: 48-Byte `NotePlaintext` & 96-Byte AEAD Memo

Pada [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-037-note-encryption-memo-and-viewing-key-recovery.md), memo note terenkripsi dipatok tepat **96 byte**.

Struktur 48-byte `NotePlaintext` diperluas secara backward-compatible:
```text
┌────────────────────────────────────────────────────────────────────────┐
│                   DEC-037 48-BYTE NOTE PLAINTEXT LAYOUT                 │
├──────────────┬──────────────┬──────────────┬───────────────────────────┤
│ Offset (B)   │ Ukuran (B)   │ Tipe         │ Deskripsi                 │
├──────────────┼──────────────┼──────────────┼───────────────────────────┤
│ 00 .. 07     │ 8            │ u64 (LE)     │ Note Value (Gwei / Micro) │
│ 08 .. 39     │ 32           │ [u8; 32]     │ Note Nullifier Seed (rho) │
│ 40 .. 43     │ 4            │ u32 (LE)     │ Epoch ID                  │
│ 44           │ 1            │ u8           │ Asset ID (0: WETH, 1:USDC)│
│ 45 .. 47     │ 3            │ [u8; 3]      │ Reserved (Zero Padding)   │
└──────────────┴──────────────┴──────────────┴───────────────────────────┘
```

Saat dompet memulihkan note via Incoming Viewing Key (`recover_notes`):
1. Dekripsi ChaCha20Poly1305 menghasilkan buffer 48 byte.
2. SDK membaca `asset_id` di byte ke-44.
3. SDK secara instan mengarahkan saldo ke sub-wallet yang tepat (USDC atau WETH) tanpa perlu query tambahan ke blockchain.

---

## 6. Normalisasi Skala Satuan (18 Desimal WETH vs 6 Desimal USDC)

1. **USDC (6 Desimal):** 1 USDC = $1{,}000{,}000$ mikro-unit. Nilai $100{,}000\text{ USDC} = 10^{11}$ pas dalam rentang `u64`.
2. **WETH (18 Desimal):** Satuan dasar wei ($10^{18}$) rentan overflow jika dimasukkan ke integer 64-bit ($2^{64}-1 \approx 18.44 \times 10^{18}$).
3. **Standarisasi Gwei (9 Desimal):**
   $$1\text{ Gwei} = 10^9\text{ Wei}$$
   Di dalam sirkuit dan private note wallet, seluruh nilai WETH disimpan dan dibuktikan dalam satuan **Gwei**:
   - Nilai minimum deposit: $0.005\text{ WETH} = 5{,}000{,}000\text{ Gwei}$.
   - Kapasitas maksimum note: $18{,}446{,}744{,}073\text{ Gwei} \approx 18.4\text{ Miliar WETH}$.
   - Konversi Wei $\leftrightarrow$ Gwei dilakukan di boundary transfer on-chain Stylus.

---

## 7. Matriks Pengujian Formal & Negative Test Suite

1. `test_bit_packing_roundtrip_soundness`: Enkoding dan dekoding `(value, asset_id)` pada field $\text{Fr}$ menghasilkan nilai identik.
2. `test_asset_id_boolean_constraint_rejects_greater_than_one`: Bukti dengan `asset_id = 2` atau elemen acak lainnya ditolak oleh R1CS constraint.
3. `test_intra_asset_conservation_rejects_cross_spend`: Input note WETH dengan output commitment USDC memicu `SynthesisError::UnconstrainedVariable` / pembatalan verifikasi proof.
4. `test_dec037_note_plaintext_roundtrip_with_asset_id`: Serialisasi dan dekripsi memo 96-byte berhasil mengembalikan `asset_id` dengan benar.
5. `test_gwei_normalization_precision_bounds`: Pengujian batas atas dan batas bawah konversi Wei-Gwei tanpa sisa modulo.
