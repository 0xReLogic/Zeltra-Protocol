# Nimbus Core — 01: Primitives & Kurva Kriptografi

Dokumen ini menjelaskan fondasi kriptografi, kurva pairing, dan pemetaan hash yang diimplementasikan pada crate [`nimbus-core`](file:///workspaces/Zeltra-Protocol/nimbus-core).

---

## 1. Peran `nimbus-core`

`nimbus-core` adalah mesin kriptografi sentral yang menyediakan:
1. Primitif **BLS12-381** untuk tanda tangan terbutakan (Blind Signatures) dan Threshold Signatures.
2. Formatter calldata untuk precompile EVM **EIP-2537** di Arbitrum Stylus.
3. Struktur **Private Note V1 (UTXO)** dan Poseidon Merkle Tree (kedalaman 20).
4. Sirkuit ZK-SNARK **Groth16** (`PrivateNoteCircuit`) untuk pembuktian kepemilikan note dan nilai kembalian (*change output*).
5. State machine akuntansi multi-liability (`ContractAccounting`) dengan pembuktian solvabilitas (*solvency invariant*).

---

## 2. Kurva Eliptik: BLS12-381

Nimbus menggunakan kurva pairing-friendly **BLS12-381** yang didukung oleh ekosistem **Arkworks 0.6.0** (`ark-bls12-381`):

| Grup Kurva | Tipe Data Arkworks | Ukuran Terkompresi | Peruntukan di Nimbus |
|---|---|---|---|
| **$G_1$** | `G1Projective` / `G1Affine` | 48 bytes (128 bytes EVM) | Blinded Message ($X$), Masked Signature ($\tilde{\sigma}$), Final Signature ($\alpha$) |
| **$G_2$** | `G2Projective` / `G2Affine` | 96 bytes (256 bytes EVM) | Issuer Public Key ($pk_{iss}$), Masking Key Commitment ($com_k$) |
| **$G_T$** | `Fq12` | 576 bytes | Target grup operasi pairing bilinear $e: G_1 \times G_2 \to G_T$ |
| **$\text{Fr}$** | `Fr` (Scalar field) | 32 bytes (255 bits) | Blinding Factor ($r$), Masking Key ($k$), Secret Key ($sk_{iss}$), Private Note secrets |

### Keunggulan Pemilihan BLS12-381:
* **Tingkat Keamanan 128-bit:** Menahan serangan discrete log modern (SNFS) dengan ukuran grup optimal.
* **Kompatibilitas On-Chain EVM:** Didukung secara native di Arbitrum via precompile **EIP-2537** (alamat `0x0b` s/d `0x13`).

---

## 3. Hash-to-Curve $G_1$ (RFC 9380)

File referensi: [`nimbus-core/src/crypto.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/crypto.rs)

Untuk memverifikasi tanda tangan BLS, pesan plaintext harus dipetakan ke titik sah pada grup kurva $G_1$. Nimbus mengimplementasikan **RFC 9380 standar** menggunakan ciphersuite:
```text
BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_
```
* **Metode:** MapToCurve Wahby-Boneh (Simplified Shallue-van de Woestijne-Ulas / SSWU).
* **Fungsi Kode:** `hash_to_g1(message: &[u8]) -> G1Projective`.
* **Keamanan:** Melindungi protokol dari serangan relasi discrete-log antar hash point karena menghasilkan titik acak semu yang seragam pada grup prima $G_1$.

---

## 4. EVM Precompile Data Transport (EIP-2537)

File referensi: [`nimbus-core/src/evm.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/evm.rs)

Precompile EIP-2537 pada Ethereum/Stylus membutuhkan koordinat uncompressed Big-Endian field elements:
* Titik $G_1 = (x, y)$ membutuhkan **128 bytes** (masing-masing koordinat $x, y \in \mathbb{F}_q$ berukuran 64 bytes zero-padded dari 48 bytes sebenarnya).
* Titik $G_2 = (x, y)$ membutuhkan **256 bytes** (masing-masing koordinat adalah elemen $\mathbb{F}_{q^2}$ berukuran $2 \times 64 = 128$ bytes).

Helper fungsi di `evm.rs`:
* `get_alpha_neg_evm(&sig)`: Menghasilkan negated signature $-\alpha \in G_1$ sepanjang 128 bytes agar pairing check di EVM menjadi:
  $$e(-\alpha, G_2) \cdot e(H(m), pk_{iss}) == 1$$
* `get_hm_evm(&msg)`: Menghasilkan representasi 128 bytes dari $H(m) \in G_1$.
* `get_pk_iss_evm(&pk)`: Menghasilkan representasi 256 bytes dari $pk_{iss} \in G_2$.

---

## 5. Poseidon Hash & Grain-128 LFSR Parameter Generator (DEC-021)

File referensi: [`nimbus-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/poseidon.rs)

Poseidon adalah fungsi hash ramah ZK (*arithmetization-oriented*) yang digunakan secara intensif di Nimbus:
* **Width $t=3$:** Digunakan untuk hashing 2 input (misal derivasi `nullifier_key` dan sirkuit compliance).
* **Width $t=5$:** Digunakan untuk komitmen note (`value, owner_key, rho, randomness`), derivasi nullifier berindeks (`nullifier_key, commitment, leaf_index`), dan parent hash pohon LeanIMT Merkle tree.
* **Standard Grain-128 LFSR (`GrainLfsr`):** Menggantikan generator PRNG ad-hoc dengan generator resmi standar Grassi et al. dan Aztec Barretenberg. Generator ini membentengi cipher dari celah *invariant subspace trails* dan serangan aljabar basis Gröbner sesuai riset *Ethereum Foundation Poseidon Cryptanalysis 2024–2026* ([`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-audited-poseidon-parameters-grain-lfsr-defense.md)).

