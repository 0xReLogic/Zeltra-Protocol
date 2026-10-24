# TASK-DEC-050: Master Engineering Implementation Guide
## Streaming Bounded-Memory Groth16 Prover via Split-Butterfly-Merge (SBM)

> **Dokumen Panduan Teknis untuk AI Agent & Cryptographic Engineers**  
> Dokumen ini adalah instruksi operasional langkah demi langkah untuk mengimplementasikan **DEC-050** ke dalam codebase Zeltra Protocol. Ikuti seluruh aturan, struktur modul, penamaan fungsi, dan gerbang verifikasi (Test Gates) tanpa deviasi.

---

## 1. Metadata & Referensi Induk

* **Dokumen Arsitektur Resmi:** [`research/dec/DEC-050-streaming-bounded-memory-groth16-prover-and-sluice-sbm-architecture.md`](research/dec/DEC-050-streaming-bounded-memory-groth16-prover-and-sluice-sbm-architecture.md)
* **Paper Ilmiah Acuan:** IACR Cryptology ePrint 2026/1758 (*"Sluice: Prove-Phase Bounded-Memory Groth16 via Read-Write Streaming"*, Oktober 2026, Kyeongtae Lee, Jihye Kim, Hyunok Oh) di [`jurnal/pdf/2026-1758.pdf`](jurnal/pdf/2026-1758.pdf).
* **Target Crates:**
  * `zeltra-core` (Pembuatan sub-modul baru `src/sbm/` dan engine pembuktian)
  * `zeltra-sdk` (Integrasi Web Worker WASM & browser OPFS di `src/zk_wasm.rs`)
  * `zeltra-contracts` (**MUTLAK 100% UNCHANGED** — Verifier Stylus EIP-2537 tidak boleh disentuh)
* **Hasil Pengukuran Riil Baseline Mesin (Terukur Linux `ru_maxrss`):**
  * Sirkuit: `JoinSplitCircuit` ($d = 34,571$ R1CS constraints, $N = 2^{16} = 65,536$, 19 Public Inputs)
  * Durasi Pembuktian Standar: **~4.97–5.00 detik** (`target/release/bench`)
  * Durasi Verifikasi: **6.45–6.86 ms** (155 ops/sec)
  * Puncak Alokasi RAM Monolitik Standar: **245.80 MiB** (251,704 KB Max RSS)
  * Target Puncak RAM SBM: **~11.8 MiB** ($O(\log N)$ working state)

---

## 2. Invariants & Batasan Non-Negotiable (BACA SEBELUM CODING)

1. **Proof Invariance (SOUND-01):**
   Bukti yang dihasilkan oleh prover SBM $\pi_{\text{SBM}} = ([A]_1, [B]_2, [C]_1)$ **wajib byte-for-byte identik** dengan bukti yang dihasilkan oleh prover standar Arkworks `Groth16::<Bls12_381>::prove` ketika diberikan pasangan sirkuit, saksi, proving key, dan keacakan $(r, s)$ yang sama:
   $$\pi_{\text{SBM}} \equiv \pi_{\text{Arkworks}}$$
2. **Zero Stylus Smart Contract Modification (VERIF-01):**
   Smart contract Arbitrum Stylus (`zeltra-contracts/src/groth16_joinsplit_verifier.rs` dan `groth16_note_verifier.rs`) menggunakan precompile EIP-2537 (`0x0c` MSM + `0x0f` 4-pairing). **DILARANG MERUBAH APAPUN DI CRATE CONTRACTS**.
3. **Preservasi 100% Layout Wire & Public Inputs (WIRE-01):**
   * `JoinSplitCircuit` memiliki tepat **19 Public Inputs** (`PI[0..18]`):
     `note_root`, `leaf_count`, `input_nullifier_1`, `input_nullifier_2`, `epoch_id_1`, `epoch_id_2`, `output_commitment_1`, `output_commitment_2`, `recipient`, `merchant_amount`, `protocol_fee`, `execution_fee`, `quote_hash_hi`, `quote_hash_lo`, `chain_id`, `contract_address`, `expiry`, `flags_packed`, `is_rollover`.
   * `PrivateNoteCircuit` memiliki tepat **16 Public Inputs** (`PI[0..15]`).
   * Jangan mengubah urutan wire atau menyuntikkan wire dummy baru di antarmuka publik.
4. **Preservasi DEC-046 & DEC-047:**
   Seluruh constraint pendamping kuadratik anti-null branch (`is_dummy * (val - dummy) == 0`) dan evaluasi rank Augmented Jacobian $r_{T} = 0$ tetap dipenuhi oleh saksi R1CS.
5. **Private Storage & Memory Cleanup:**
   Semua file stream perantara wajib berada di folder privat terisolasi, di-unlink saat selesai, dan memori di-zeroize (`zeroize::Zeroize`).

---

## 3. Struktur Modul Baru di `zeltra-core`

Buat direktori baru: `zeltra-core/src/sbm/` dengan pembagian file modular berikut:

```text
zeltra-core/src/sbm/
├── mod.rs               // Ekspor publik, konfigurasi SBM, error enum
├── stream.rs            // Trait ReadWriteStream, FileStream, dan MemoryChunkStream
├── twiddle.rs           // Generator Twiddle Factor On-The-Fly (Lemma 3.4 Sluice)
├── rw_ntt.rs            // Split-Butterfly-Merge engine & streaming BitSwap
├── chunked_msm.rs       // Chunked Pippenger MSM accumulator over BLS12-381 G1/G2
├── mat_vec.rs           // Streaming sparse R1CS matrix-vector multiplication
├── quotient.rs          // Pipeline 7-pass quotient polynomial h(X)
└── prover.rs            // High-level API: generate_joinsplit_proof_sbm & generate_note_proof_sbm
```

---

## 4. Rincian Teknis Implementasi (Step-by-Step)

### Task 1: Trait Abstraksi Stream (`zeltra-core/src/sbm/stream.rs`)
Definisikan trait baca-tulis sekuensial sesuai model Read-Write Streaming (Definition 2.5 Sluice):

```rust
use std::io::{self, Read, Seek, Write};

pub trait ReadWriteStream<T>: Send + Sync {
    /// Menulis elemen ke akhir stream
    fn write_element(&mut self, elem: &T) -> io::Result<()>;
    /// Membaca elemen berikutnya secara sekuensial
    fn read_element(&mut self) -> io::Result<Option<T>>;
    /// Mereset kursor pembacaan kembali ke awal stream (Pass baru)
    fn restart(&mut self) -> io::Result<()>;
    /// Mengetahui jumlah elemen dalam stream
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
    /// Membersihkan dan menghapus data stream
    fn truncate(&mut self) -> io::Result<()>;
}
```

* **Dua Implementasi Wajib:**
  1. `MemoryChunkStream<T>`: Menggunakan chunk buffer vektor berukuran terbatas di RAM (sangat berguna untuk unit testing dan environment non-filesystem).
  2. `FileStream<T>`: Menggunakan `std::fs::File` dengan buffer I/O 256 KiB (`std::io::BufReader` / `BufWriter`) untuk operasi disk nyata / browser OPFS.

---

### Task 2: Twiddle Generator On-The-Fly (`zeltra-core/src/sbm/twiddle.rs`)
Implementasikan Lemma 3.4 Sluice tanpa menggunakan tabel memori $O(N)$:

```rust
use ark_bls12_381::Fr;
use ark_ff::{Field, PrimeField};

#[derive(Clone, Debug)]
pub struct TwiddleGenerator {
    omega_cur: Fr,
    step_constant: Fr,
    stride: usize,
    counter: usize,
}

impl TwiddleGenerator {
    pub fn new(primitive_root: Fr, n: usize, j: usize) -> Self {
        let stride = 1usize << j;
        let exponent = 1u64 << (n - j - 1);
        let step_constant = primitive_root.pow(&[exponent]);
        Self {
            omega_cur: Fr::from(1u64),
            step_constant,
            stride,
            counter: 0,
        }
    }

    #[inline(always)]
    pub fn next_twiddle(&mut self) -> Fr {
        let current = self.omega_cur;
        self.counter += 1;
        if self.counter % self.stride == 0 {
            self.omega_cur = Fr::from(1u64); // Reset setiap 2^j langkah
        } else {
            self.omega_cur *= self.step_constant;
        }
        current
    }
}
```

---

### Task 3: Engine Split-Butterfly-Merge NTT (`zeltra-core/src/sbm/rw_ntt.rs`)
Implementasikan siklus 3-pass SBM per tahap butterfly $j$ (Section 3.3 Sluice):

1. **`split_pass<S>(input: &mut S, sa: &mut S, sb: &mut S, n: usize, j: usize)`:**
   * Scan sekuensial elemen $p = 0 \dots N-1$.
   * Jika bit ke-$j$ bernilai 0 (`(p >> j) & 1 == 0`), tulis ke `sa`.
   * Jika bit ke-$j$ bernilai 1, tulis ke `sb`.
2. **`paired_bfly_pass<S>(sa: &mut S, sb: &mut S, oa: &mut S, ob: &mut S, twiddle: &mut TwiddleGenerator)`:**
   * Baca `sa` dan `sb` secara lockstep (baca sepasang $a_q$ dan $b_q$).
   * Hitung:
     $$y_q^{\text{top}} = a_q + \omega_q \cdot b_q \implies \text{tulis ke } oa$$
     $$y_q^{\text{bot}} = a_q - \omega_q \cdot b_q \implies \text{tulis ke } ob$$
3. **`merge_pass<S>(oa: &mut S, ob: &mut S, output: &mut S, n: usize, j: usize)`:**
   * Rekonstruksi urutan alami dengan membaca blok berukuran $2^j$ secara bergantian:
     * Baca $2^j$ elemen dari `oa`, tulis ke `output`.
     * Baca $2^j$ elemen dari `ob`, tulis ke `output`.
4. **`streaming_bitrev<S>(stream: &mut S, n: usize)`:**
   * Lakukan $\lfloor n/2 \rfloor$ iterasi `bitswap(stream, k, n - 1 - k)` menggunakan 4 stream bantu $S_{00}, S_{01}, S_{10}, S_{11}$.

---

### Task 4: Chunked Pippenger MSM (`zeltra-core/src/sbm/chunked_msm.rs`)
Implementasikan akumulator MSM sekuensial (Section 4.2 Sluice):

1. Parameter window chunk:
   $$w = \min(16, \max(1, \lfloor \log_2(\log_2 N + 1) \rfloor + 1))$$
   Untuk $N = 2^{16} = 65,536$, gunakan $w = 5 \implies B_{\text{MSM}} = 32$.
2. Baca chunk $B_{\text{MSM}}$ pasangan skalar dan titik kurva affine.
3. Evaluasi partial MSM di RAM via Pippenger standar.
4. Akumulasikan ke titik projektif:
   $$\text{acc} \leftarrow \text{acc} + P_{\text{chunk}}$$
5. Di akhir stream, normalisasikan `acc.into_affine()` menjadi titik kurva affine canonical.

---

### Task 5: Pipeline Evaluasi Polinomial Quotient $h(X)$ (`zeltra-core/src/sbm/quotient.rs`)
Lakukan pipeline 7-pass transform sesuai Section 5.3 Sluice:
1. Jalankan `RW-iNTT` pada evaluasi R1CS $\hat{a}, \hat{b}, \hat{c}$ untuk memperoleh koefisien $F_U(X), F_V(X), F_W(X)$.
2. Lakukan pergeseran coset $f'_i = f_i \cdot \eta^i$ di mana $\eta$ adalah generator coset ($\eta^N \neq 1$, gunakan generator subgrup akar $2^{28}$ BLS12-381).
3. Jalankan `RW-NTT` pada masing-masing polinomial coset.
4. Hitung pembagian titik demi titik (*pointwise quotient*):
   $$\hat{h}_j = \frac{F_U(\eta \omega^j) \cdot F_V(\eta \omega^j) - F_W(\eta \omega^j)}{\eta^N - 1}$$
5. Jalankan `RW-iNTT` pada $\hat{h}$ dan lakukan *unshift* $h_i = c_i \cdot \eta^{-i}$.
6. Hasilnya adalah koefisien polinomial kuosien $h(X)$ berderajat $\le N - 2$.

---

### Task 6: Integrasi High-Level Prover (`zeltra-core/src/sbm/prover.rs`)
Sediakan API publik yang drop-in compatible:

```rust
pub fn generate_joinsplit_proof_sbm(
    circuit: JoinSplitCircuit,
    pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<ark_groth16::Proof<Bls12_381>, SynthesisError>;

pub fn generate_note_proof_sbm(
    circuit: PrivateNoteCircuit,
    pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<ark_groth16::Proof<Bls12_381>, SynthesisError>;
```

---

## 5. Gerbang Pengujian & Verifikasi (Test Gates)

Sesuai aturan protokol Zeltra, **rasio pengujian negatif wajib memenuhi $\ge 2\times$ pengujian positif**:

* **Gate 1 (Positive KAT Verification):**
  * Uji komparasi deterministik: Bangun saksi `create_dummy_joinsplit_circuit()` dengan fixed RNG seed (`0x4e696d6275734a53`).
  * Jalankan `generate_joinsplit_proof` (Standar Arkworks) dan `generate_joinsplit_proof_sbm` (SBM).
  * Assert equality byte-per-byte: `proof_sbm.a == proof_std.a`, `proof_sbm.b == proof_std.b`, `proof_sbm.c == proof_std.c`.
  * Verifikasi `verify_joinsplit_proof(&vk, &proof_sbm, &public_inputs)` mengembalikan `Ok(true)`.
* **Gate 2 (Negative Adversarial Tests — Min. 10 Tests):**
  1. `test_corrupted_split_stream_rejected`: Modifikasi bit pada stream $S_A$, buktikan hasil verifikasi gagal.
  2. `test_truncated_r1cs_stream_fails`: Potong stream matriks R1CS di tengah jalan, buktikan error `UnexpectedEof` tertangkap fail-closed.
  3. `test_non_canonical_scalar_sbm_rejected`: Suntikkan skalar $\ge r$ ke stream witness, buktikan ditolak.
  4. `test_tampered_twiddle_step_fails`: Geser konstanta $W_j$, buktikan pairing verification smart contract gagal.
  5. `test_invalid_coset_generator_rejected`: Gunakan generator coset $\eta$ dengan $\eta^N = 1$, buktikan pembagian quotient divide-by-zero dicegah.
  6. `test_chunked_msm_bucket_overflow`: Uji batas window chunk $w > 16$, pastikan ter-clamp aman ke batas maksimum.
  7. `test_unlinked_workspace_zeroized`: Periksa file temporary di filesystem setelah proving, buktikan sudah terhapus dan tidak ada kebocoran saksi.
  8. `test_joinsplit_sbm_tampered_execution_fee`: Modifikasi execution fee saksi (+100), buktikan pembuktian gagal.
  9. `test_joinsplit_sbm_nullifier_aliasing_rejected`: Set `nf1 == nf2` pada saksi, buktikan sirkuit menolak.
  10. `test_joinsplit_sbm_dummy_input_violation`: Manipulasi `in2_is_dummy = 1` tetapi `in2_value != 0`, buktikan fail-closed.
* **Gate 3 (Benchmark & Memory Cap Audit):**
  * Jalankan benchmark perbandingan:
    ```bash
    cargo test -p zeltra-core test_sbm_prover_memory_cap -- --nocapture
    ```
  * Buktikan working memory SBM tetap berada di bawah **16 MiB RAM**, sementara prover standar mengambil ~246 MiB.

---

## 6. Command Cheatsheet

```bash
# 1. Menjalankan pengujian sirkuit & budget konstrain
cargo test -p zeltra-core test_joinsplit_constraint_budget_audit -- --nocapture

# 2. Menjalankan pengujian unit SBM (RW-NTT, Twiddle, Chunked MSM)
cargo test -p zeltra-core sbm:: -- --nocapture

# 3. Menjalankan verifikasi kesetaraan pembuktian SBM vs Standar
cargo test -p zeltra-core test_sbm_groth16_proof_equality -- --nocapture

# 4. Menjalankan benchmark rilis lengkap di mesin
cargo run --release -p zeltra-core --bin bench
```

---
*Dokumen ini adalah spesifikasi operasional resmi Zeltra Protocol. Dilarang mengubah wire layout atau mematikan invariant keamanan sirkuit.*
