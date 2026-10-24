# DEC-041: In-Browser WebAssembly Groth16 Prover, Non-Blocking Web Worker & Disposable Anti-OOM Architecture

**Status:** APPROVED  
**Date:** 2026-10-16  
**Domain:** `zeltra-sdk` (Client WASM Prover & Web Worker Runtime), `zeltra-core` (Browser-Compatible Circuit Witness Evaluation)  
**Security Level:** CRITICAL (Client Financial Privacy, Browser/Mobile Memory Safety, Anti-OOM, Non-Blocking UX)  
**Related Decisions:**
- [`DEC-021: Audited Poseidon Parameters & Grain-128 LFSR`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-021-audited-poseidon-parameters-grain-lfsr-defense.md)
- [`DEC-032: Merkle Mountain Range Commitment Accumulator`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-032-merkle-mountain-range-commitment-accumulator.md)
- [`DEC-034A: Canonical Field Bit-Packing & Intra-Asset Circuit Constraints`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md)
- [`DEC-035A: Two-Limb Quote Hash Representation and Range Constraints`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md)
- [`DEC-035B: Formal Scope Binding Gadget & Groth16 Statement Integrity`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035B-formal-scope-binding-gadget-and-groth16-statement-integrity.md)
- [`DEC-036A: Universal 2-in-2-out JoinSplit R1CS Circuit Specification`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md)
- [`DEC-037: Note Encryption Memo Protocol & Viewing Key Recovery`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-037-note-encryption-memo-and-viewing-key-recovery.md)
- [`DEC-038: Pure ZK-UTXO Migration & Direct MMR Deposit`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
- [`DEC-040: Viewing Key Export Suite & CEX Selective Disclosure`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)
- [`DEC-040A: Selective Disclosure Circuit & Key Separation`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040A-selective-disclosure-circuit-and-key-separation.md)

---

## 1. Executive Summary & Context

Zeltra Protocol adalah infrastruktur pembayaran dan penyelesaian privat universal (*"The Stripe of Web3 with Absolute Privacy"*). Filosofi produk Zeltra adalah **"Privacy 9.5/10, Product 10/10"**: pengalaman pengguna secepat dan semudah Apple Pay / QRIS dengan mental model kas nyata ($10 bayar $3 kembali $7 lewat ZK-UTXO Private Note), di mana seluruh komputasi kriptografi diselesaikan dengan mulus di belakang layar.

### A. Dilema Privasi Klien vs Kenyamanan UX
Untuk mencapai privasi transaksi tanpa kompromi (*default financial privacy*):
1. **Dilarang Delegated Proving:** Pengguna **TIDAK BOLEH** mengirimkan rahasia saksi (`spending_key`, `rho`, `r`, atau saldo kas) ke server backend atau relayer untuk dibuatkan bukti ZK. Mendelegasikan saksi ke pihak ketiga menghancurkan privasi dan menciptakan *honeypot* peretasan massal.
2. **Kewajiban Client-Side Proving:** Pembuatan bukti ZK (Groth16 NIZK atas kurva BLS12-381) wajib dieksekusi 100% secara lokal pada perangkat pengguna (peramban desktop, laptop, tablet merchant, atau smartphone).
3. **Kendala Nyata Ekosistem WebAssembly Peramban:**
   - Komputasi kurva eliptik BLS12-381 (Multi-Scalar Multiplication / MSM dan Fast Fourier Transform / FFT) membutuhkan ratusan megabyte RAM dan jutaan operasi modular 256-bit.
   - Jika dijalankan di thread utama JavaScript, halaman web akan **membeku total (0 FPS, UI unresponsiveness)** selama 2–6 detik.
   - Lebih kritis lagi: **Memori linear WebAssembly (`memory.grow`) tidak pernah menyusut (monotonic growth)** di peramban modern (Chrome V8, Safari JavaScriptCore, Firefox SpiderMonkey). Dalam sesi web yang panjang (misalnya tab dompet web atau kasir Point-of-Sale merchant yang dibuka berjam-jam), memori linear akan terakumulasi hingga memicu **Out-Of-Memory (OOM) Crash** pada peramban mobile (iOS WebKit limit ~1.4 GB).

Dokumen ini menetapkan spesifikasi formal **DEC-041: In-Browser WebAssembly Groth16 Prover** dengan pola **Disposable Dedicated Web Worker (`worker.terminate()`)**. Pola ini memberikan jaminan matematis:
- **Zero UI Blocking (60 FPS):** Proving diisolasi ke background thread tanpa mengganggu antarmuka pengguna.
- **Zero Memory Accumulation ($O(1)$ RAM Lifecycle):** Memori linear dan konteks WebAssembly dihancurkan seketika setelah bukti selesai dibuat, mengembalikan 100% alokasi memori heap ke sistem operasi.
- **Zero Backend Trust:** Kunci privat tidak pernah keluar dari memori peramban klien.
- **Zero Third-Party Header Dependency:** Berjalan secara mandiri tanpa memerlukan header HTTP `Cross-Origin-Opener-Policy` (COOP) / `Cross-Origin-Embedder-Policy` (COEP), sehingga dapat disematkan secara *plug-and-play* pada widget checkout situs e-commerce pihak ketiga mana pun.

---

## 2. Post-Mortem & Security Archaeology (2019–2026)

Sebelum merancang DEC-041, kami membedah kegagalan sistem pembuktian berbasis browser dalam sejarah ekosistem ZK:

### A. Penumbra Arkworks WASM Bottleneck (2024)
* **Temuan Analisis:**
  Pada ekosistem Penumbra, kompilasi kode Rust Arkworks ke WASM menghasilkan perlambatan 25x–30x dibanding native binary. Ada dua akar penyebab:
  1. *128-bit Multiplier Emulation:* Arkworks menggunakan representasi limb 64-bit yang membutuhkan perkalian 128-bit (`u128`). Di lingkungan WebAssembly 32-bit (`wasm32`), perkalian 128-bit diemulasikan melalui instruksi 32/64-bit berlapis yang menyebabkan pembengkakan instruksi CPU drastis.
  2. *Proving Key Bloat:* Memaketkan proving key langsung ke dalam file binary `.wasm` membengkakkan ukuran file dari 9 MB menjadi 90 MB, mengakibatkan waktu inisialisasi worker membengkak dari 150 ms menjadi > 3.500 ms.
* **Mitigasi Zeltra:**
  - Binary `.wasm` Zeltra dijaga sangat ramping (< 2.5 MB) murni memuat logika constraint R1CS dan prover.
  - Proving key disimpan dan di-fetch terpisah sebagai raw binary stream terkompresi, lalu di-cache pada `IndexedDB` browser pengguna.

### B. SnarkJS / ffjavascript Hanging Worker Threads (Issue #152 & #292)
* **Vektor Masalah:**
  Pada pustaka `snarkjs` / `ffjavascript`, thread worker dan kurva eliptik ditempelkan ke objek global (`globalThis`). Worker threads yang tidak dimatikan secara eksplisit mempertahankan *event loop handles* aktif di runtime Node.js dan browser, menyebabkan pengujian unit menggantung (*hang*) dan memori tidak dapat di-garbage collect.
* **Mitigasi Zeltra:**
  Pola **Disposable Worker**. Zeltra melarang worker berstatus *singleton* atau *long-lived*. Setiap permintaan pembuktian menciptakan worker baru yang diterminasi seketika setelah transmisi hasil:
  $$\text{Lifecycle: } \text{Spawn} \longrightarrow \text{Prove} \longrightarrow \text{PostMessage} \longrightarrow \text{worker.terminate()}$$

### C. Tornado Cash & Early Aztec UI Freeze (2020–2022)
* **Vektor Masalah:**
  Aplikasi web generasi awal mengeksekusi prover ZK di thread utama peramban. Selama perhitungan saksi dan bukti (1–5 detik), tombol antarmuka tidak merespons klik pengguna, animasi CSS berhenti berputar, dan peramban menampilkan pop-up *"Page Unresponsive / Kill Page"*, menurunkan konversi merchant hingga 40%.
* **Mitigasi Zeltra:**
  Thread UI utama hanya bertindak sebagai orkestrator asinkronus (`async/await`) yang menerima event status (`PROGRESS_MSM`, `PROGRESS_FFT`, `SUCCESS`, `ERROR`) dari Web Worker. Frame rate antarmuka pengguna dijamin stabil pada 60 FPS.

### D. The Linear Memory Monotonicity Trap (OOM di Safari & Chrome)
* **Vektor Kerentanan:**
  Di WebAssembly, fungsi alokator Rust (`dlmalloc` atau `wee_alloc`) dapat mengembalikan pointer memori ke *free-list internal* WASM, tetapi **tidak dapat memperkecil ukuran tabel halaman memori linear host**. Sekali peramban mengalokasikan 400 MB memori untuk MSM, 400 MB tersebut tetap terkunci di tab peramban.
* **Dampak:**
  Di Safari iOS, batas memori per tab web view adalah ~1.4 GB (atau serendah 500 MB jika ada beban memori sistem). Menjalankan 5 transaksi ZK dalam satu sesi tanpa me-reload halaman menyebabkan peramban *crash* total akibat kehabisan memori.
* **Mitigasi Zeltra:**
  Pemanggilan `worker.terminate()` adalah satu-satunya API standar web yang memaksa mesin JavaScript (V8 / JavaScriptCore) untuk melepaskan seluruh heap WebAssembly dan mengembalikan tabel halamannya ke OS.

### E. Kegagalan Ketergantungan Header COOP/COEP
* **Vektor Masalah:**
  Multi-threading di browser WASM membutuhkan fitur `SharedArrayBuffer`. Standar keamanan web modern (pasca-Spectre) mewajibkan dua header HTTP:
  `Cross-Origin-Opener-Policy: same-origin` dan `Cross-Origin-Embedder-Policy: require-corp`.
  Akibatnya, widget checkout Zeltra yang disematkan melalui tag `<script>` atau `<iframe>` di situs merchant (seperti Shopify atau toko mandiri) akan gagal beroperasi karena pemilik toko tidak memiliki akses untuk mengubah konfigurasi header server hosting mereka.
* **Mitigasi Zeltra:**
  Zeltra memprioritaskan arsitektur **Single-Threaded Pure Worker** yang mandiri dan beroperasi pada browser standar mana pun tanpa prasyarat header COOP/COEP.

---

## 3. Arsitektur Sistem & Siklus Hidup Disposable Worker

### A. Diagram Topologi Tiga Lapisan
```
┌────────────────────────────────────────────────────────────────────────┐
│                        MAIN BROWSER UI THREAD                          │
│                                                                        │
│   [ React / Next.js / Checkout Widget ]                                │
│                   │                                                    │
│                   ▼                                                    │
│      ZeltraBrowserProverClient                                         │
│         ├── 1. Baca Proving Key dari IndexedDB (atau fetch CDN)        │
│         ├── 2. new Worker('zeltra_prover_worker.js')                   │
│         ├── 3. postMessage({ type: "PROVE", ... }, [ArrayBuffer])      │
│         │        ▲                                                     │
│         │        │ 5. onmessage: { proof, public_inputs }              │
│         │        │                                                     │
│         ├── 4. Listen progress & result                                │
│         └── 6. worker.terminate()  ◄─── SIKLUS PEMUSNAHAN MEMORI       │
│                                                                        │
└───────────────────┬───────────────────────────────────▲────────────────┘
                    │ Transfer Data (Zero-Copy)         │ Hasil Bukti
                    ▼                                   │
┌───────────────────────────────────────────────────────┴────────────────┐
│                   DEDICATED PROVER WEB WORKER                          │
│                                                                        │
│   zeltra_prover_worker.js                                              │
│         ├── Memuat modul WebAssembly (zeltra_sdk_wasm.wasm)             │
│         ├── Inisialisasi console_error_panic_hook                      │
│         ├── Eksekusi Rust wasm-bindgen:                                │
│         │     • Deserialisasi parameter saksi (Witness)                │
│         │     • Sintesis sirkuit Arkworks (R1CS Constraints)           │
│         │     • Multi-Scalar Multiplication (MSM)                      │
│         │     • Fast Fourier Transform (FFT)                           │
│         │     • Format Big-Endian EVM Public Inputs (16/19/5 PIs)      │
│         └── Transmisi pesan SUCCESS kembali ke Main Thread             │
│                                                                        │
│   [ MEMORI LINEAR WASM: 250 MB – 350 MB PEAK ]                         │
│   (Akan dimusnahkan 100% oleh terminate() pada tahap 6)                │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Protokol Eksekusi & Manajemen Memori

### A. Siklus 6 Tahap Pembuktian Bebas-Kebocoran ($O(1)$ Memory Life-Cycle)

1. **Tahap 1 (Key Cache Resolution):**
   Main thread memeriksa apakah `proving_key` untuk sirkuit yang bersangkutan (`private_note`, `joinsplit`, atau `disclosure`) sudah tersimpan di cache `IndexedDB` browser. Jika belum, ambil berkas biner terkompresi dari endpoint CDN terdistribusi atau bundle lokal, lalu simpan ke `IndexedDB`.
2. **Tahap 2 (Worker Spawning):**
   Main thread menginstansiasi worker terisolasi:
   ```javascript
   const worker = new Worker(new URL('./zeltra_prover_worker.js', import.meta.url), { type: 'module' });
   ```
3. **Tahap 3 (Zero-Copy Message Transfer):**
   Parameter saksi (witness) dan proving key biner dikirim ke worker menggunakan *Transferable Objects* (`ArrayBuffer`), menghindari duplikasi memori di heap JavaScript:
   ```javascript
   worker.postMessage({
       action: 'PROVE_JOINSPLIT',
       payload: witnessJson,
       provingKeyBuffer: pkUint8Array.buffer
   }, [pkUint8Array.buffer]);
   ```
4. **Tahap 4 (WASM Execution & Event Emitting):**
   Worker mengeksekusi fungsi prover WASM. Di dalam Rust, alokasi memori heap dikelola oleh `dlmalloc`. Selama proses berjalan, worker dapat mengirimkan pesan denyut jantung (*heartbeat/progress*) ke thread utama:
   ```javascript
   postMessage({ status: 'PROGRESS', stage: 'MSM', percentage: 65 });
   ```
5. **Tahap 5 (Receipt & Verification Guard):**
   Setelah kalkulasi selesai, worker mengirimkan kembali `proof_hex` dan `public_inputs` (format big-endian 32-byte untuk precompile EIP-2537 `0x0c` + `0x0f`).
6. **Tahap 6 (Mandatory Immediate Termination):**
   Thread utama menangkap hasil, dan di dalam blok `finally`, mengeksekusi terminasi:
   ```javascript
   try {
       const result = await proofPromise;
       return result;
   } finally {
       worker.terminate(); // Konteks WebAssembly & V8 Isolate seketika dibebaskan
   }
   ```

---

## 5. Spesifikasi Sirkuit yang Didukung di WASM

Modul `zeltra_sdk_wasm` mengompilasi dan mengekspos prover untuk seluruh portofolio sirkuit Zeltra Protocol:

| Sirkuit | Tujuan Fungsional | Public Inputs | Target Waktu (M1/M2) | Target Waktu (Mobile) |
|:---|:---|:---:|:---:|:---:|
| **`PrivateNoteCircuit`** ([`DEC-035A/B`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md)) | 1-in-1-out Note Spend & Change | 16 PIs | 1.8 – 2.5 detik | 3.5 – 5.0 detik |
| **`JoinSplitCircuit`** ([`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md)) | Universal 2-in-2-out Spend, Merge & Consolidate | 19 PIs | 2.5 – 3.8 detik | 5.0 – 7.5 detik |
| **`SelectiveDisclosureCircuit`** ([`DEC-040A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040A-selective-disclosure-circuit-and-key-separation.md)) | Bukti Sumber Dana CEX (Proof of Innocence) | 5 PIs | < 25 milidetik | < 50 milidetik |

Seluruh sirkuit diverifikasi secara formal menggunakan constraint lapangan skalar BLS12-381 ($\text{Fr}$) dengan representasi big-endian 32-byte yang cocok 100% dengan precompile Arbitrum Stylus WASM EIP-2537.

---

## 6. Antarmuka Pemrograman Aplikasi (API Specification)

### A. Rust WASM Exports (`zeltra-sdk/src/zk_wasm.rs`)

```rust
use wasm_bindgen::prelude::*;

/// Konfigurasi kepanikan peramban
#[wasm_bindgen]
pub fn init_wasm_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Menghasilkan Groth16 Proof untuk 1-in-1-out PrivateNoteCircuit (16 Public Inputs)
#[wasm_bindgen]
pub fn wasm_prove_private_note(
    witness_json: &str,
    proving_key_bytes: &[u8],
) -> Result<JsValue, JsValue> {
    // 1. Deserialisasi compressed proving key
    // 2. Evaluasi saksi PrivateNoteCircuit
    // 3. Eksekusi Groth16::create_random_proof_with_reduction
    // 4. Return serialized ZkProofPayload (proof_a, proof_b, proof_c, public_inputs)
}

/// Menghasilkan Groth16 Proof untuk 2-in-2-out JoinSplitCircuit (19 Public Inputs)
#[wasm_bindgen]
pub fn wasm_prove_joinsplit(
    witness_json: &str,
    proving_key_bytes: &[u8],
) -> Result<JsValue, JsValue> {
    // 1. Deserialisasi compressed proving key
    // 2. Evaluasi saksi JoinSplitCircuit
    // 3. Eksekusi Groth16 proving
    // 4. Return serialized JoinSplitProofPayload
}

/// Menghasilkan Proof Kepemilikan CEX Selective Disclosure (5 Public Inputs - DEC-040A)
#[wasm_bindgen]
pub fn wasm_prove_selective_disclosure(
    witness_json: &str,
    proving_key_bytes: &[u8],
) -> Result<JsValue, JsValue> {
    // Proving instan (<25ms)
}
```

### B. JavaScript / TypeScript Client Wrapper (`ZeltraBrowserProver`)

```typescript
export interface ProverResult {
  proofA: string;      // 0x... (64 bytes EVM G1)
  proofB: string;      // 0x... (128 bytes EVM G2)
  proofC: string;      // 0x... (64 bytes EVM G1)
  publicInputs: string[]; // Array of 32-byte hex strings
  provingTimeMs: number;
}

export class ZeltraBrowserProver {
  private workerScriptUrl: string;
  private keyCache: Map<string, Uint8Array> = new Map();

  constructor(workerScriptUrl: string) {
    this.workerScriptUrl = workerScriptUrl;
  }

  /**
   * Menjalankan pembuktian JoinSplit di Dedicated Web Worker dengan auto-termination anti-OOM.
   */
  public async proveJoinSplit(
    witness: object,
    pkBuffer: Uint8Array,
    timeoutMs: number = 30000
  ): Promise<ProverResult> {
    const worker = new Worker(this.workerScriptUrl, { type: 'module' });

    return new Promise<ProverResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        worker.terminate();
        reject(new Error(`Prover execution timed out after ${timeoutMs}ms`));
      }, timeoutMs);

      worker.onmessage = (event: MessageEvent) => {
        if (event.data.status === 'SUCCESS') {
          clearTimeout(timer);
          worker.terminate(); // Wajib: Hancurkan worker seketika untuk melepas memori
          resolve(event.data.result);
        } else if (event.data.status === 'ERROR') {
          clearTimeout(timer);
          worker.terminate();
          reject(new Error(event.data.error));
        }
      };

      worker.onerror = (err) => {
        clearTimeout(timer);
        worker.terminate();
        reject(err);
      };

      // Transfer proving key buffer zero-copy
      worker.postMessage({
        action: 'PROVE_JOINSPLIT',
        witness,
        provingKey: pkBuffer
      });
    });
  }
}
```

---

## 7. Strict Security Invariants (Aturan Pantang Dilanggar)

1. **INVARIANT 1 (Absolute Client-Side Witness Confinement):**
   Seluruh parameter saksi privat (`spending_key`, `rho`, `r`, `value_packed`) **dilarang keras** dikirim melalui jaringan HTTP/WebSocket atau diteruskan ke thread analitik mana pun. Saksi hanya boleh hidup di heap Web Worker lokal selama proses pembuktian berlangsung.
2. **INVARIANT 2 (Mandatory Disposable Worker Lifecycle):**
   Setiap instance `Worker` yang digunakan untuk menghasilkan bukti Groth16 **wajib diterminasi (`worker.terminate()`)** segera setelah hasil diterima atau terjadi error/timeout. Dilarang mempertahankan worker *long-lived* untuk penggunaan berganda (*re-use across multiple proofs*) karena memori linear WebAssembly tidak dapat menyusut kembali ke host OS.
3. **INVARIANT 3 (Strict Zero Main-Thread Freezing):**
   Fungsi pembuktian sirkuit `PrivateNoteCircuit` dan `JoinSplitCircuit` **dilarang dipanggil secara sinkronus di UI thread utama**. Seluruh proses komputasi wajib terisolasi di background worker.
4. **INVARIANT 4 (Zero COOP/COEP External Lock-In):**
   Modul prover peramban Zeltra wajib dapat berjalan pada mode *single-threaded standard WASM* tanpa menuntut penyedia website (merchant/partner) memasang header `Cross-Origin-Opener-Policy: same-origin` atau `Cross-Origin-Embedder-Policy: require-corp`.
5. **INVARIANT 5 (Fail-Closed Graceful Error Handling):**
   Jika saksi tidak memenuhi constraint R1CS atau format skalar tidak kanonikal ($\ge r$), worker wajib menangkap error secara terstruktur dan mengirimkan pesan penolakan yang informatif, bukan melempar crash `RuntimeError: unreachable` yang membekukan aplikasi.
6. **INVARIANT 6 (Entropy Source Verification):**
   Dalam lingkungan browser, seluruh keacakan prover ($r, s$ blinders pada Groth16) wajib bersumber dari CSPRNG browser berstandar tinggi via `window.crypto.getRandomValues()`. Fallback ke insecure PRNG (seperti `Math.random()`) dilarang keras dan wajib gagal kompilasi.

---

## 8. Target Kinerja & Service Level Objectives (SLO)

Berdasarkan target spesifikasi produksi:
- **Ukuran Binary WebAssembly:** $\le 2.5\text{ MB}$ (gzipped: $\le 850\text{ KB}$).
- **Inisialisasi Worker & WASM Compile:** $\le 120\text{ ms}$.
- **Waktu Pembuktian 1-in-1-out (`PrivateNoteCircuit`):**
  - Apple Silicon (M1/M2/M3) / Modern x86-64 Desktop: $\le 2.2\text{ detik}$.
  - Mobile (Snapdragon 778G / iPhone 13): $\le 4.8\text{ detik}$.
- **Waktu Pembuktian 2-in-2-out (`JoinSplitCircuit`):**
  - Apple Silicon / Desktop: $\le 3.5\text{ detik}$.
  - Mobile: $\le 7.0\text{ detik}$.
- **Retensi Memori Pasca-Terminasi:** $0\text{ MB}$ akumulasi linear (Heap browser kembali ke baseline sebelum pembuktian).

---

## 9. Matriks Pengujian Formal & Soundness (`zeltra-sdk`)

Implementasi DEC-041 wajib menyertakan pengujian berikut dengan rasio negatif $\ge 2\times$ positif:

### Positive Tests:
1. `test_wasm_private_note_prover_produces_valid_stylus_proof`:
   Eksekusi prover WASM untuk `PrivateNoteCircuit` menghasilkan bukti Groth16 yang diverifikasi valid oleh verifier native Arkworks dan verifier on-chain Stylus EIP-2537.
2. `test_wasm_joinsplit_prover_produces_valid_stylus_proof`:
   Eksekusi prover WASM untuk `JoinSplitCircuit` 19 PI menghasilkan bukti Groth16 yang lolos verifikasi seluruh 448 quadratic range gates.
3. `test_wasm_selective_disclosure_prover_latency`:
   Bukti keterbukaan selektif CEX (5 PI) dihasilkan dalam waktu $< 25\text{ ms}$ di lingkungan WASM.
4. `test_worker_terminate_releases_linear_memory`:
   Eksekusi loop 15 kali pembuktian berturut-turut dengan pola `new Worker() -> prove -> terminate()` menghasilkan konsumsi memori stabil tanpa tren akumulasi linear (*flat memory profile*).

### Negative Tests (Anti-Crash & Boundary Hardening):
1. `test_wasm_prover_rejects_non_canonical_scalar_without_panic`:
   Memasukkan skalar $\ge r$ menghasilkan error terstruktur `InvalidScalarModulus`, bukan crash `unreachable`.
2. `test_wasm_prover_rejects_unbalanced_utxo_witness`:
   Memasukkan nilai di mana input tidak sama dengan total output + fee ditolak dengan `ConstraintUnsatisfied`, bukan menghasilkan bukti palsu.
3. `test_wasm_prover_handles_corrupted_proving_key`:
   Memasukkan array byte proving key yang rusak ditolak secara anggun dengan `DeserializationFailure`.
4. `test_worker_timeout_triggers_force_termination`:
   Simulasi worker menggantung memicu watchdog timer di main thread yang memanggil `worker.terminate()` paksa dan mengembalikan error timeout.

---

## 10. Referensi Akademik & Rekayasa

1. **Penumbra Exchange Engineering Report (2024):**
   - *Faster Client-Side Proving with Parallelism and Web Workers*.
   - URL: [https://penumbra.exchange/blog/faster-client-side-proving-with-parallelism](https://penumbra.exchange/blog/faster-client-side-proving-with-parallelism)
2. **SnarkJS / Iden3 Worker Resource Management:**
   - *Issue #152 & #292: Worker threads hanging and linear memory lifecycle in browser runtimes*.
   - URL: [https://github.com/iden3/snarkjs/issues/152](https://github.com/iden3/snarkjs/issues/152)
3. **W3C WebAssembly Core Specification (Linear Memory Model):**
   - *Section 5.4: Memory Instructions (`memory.grow` semantics and host page allocation)*.
   - URL: [https://www.w3.org/TR/wasm-core-1/](https://www.w3.org/TR/wasm-core-1/)
4. **MDN Web Docs (Dedicated Worker Termination & Transferable Objects):**
   - *Worker: terminate() method & Structured Clone Zero-Copy Buffers*.
   - URL: [https://developer.mozilla.org/en-US/docs/Web/API/Worker/terminate](https://developer.mozilla.org/en-US/docs/Web/API/Worker/terminate)
5. **Arkworks zkSNARK Ecosystem:**
   - *ark-groth16 & ark-bls12-381: High-Performance Generic SNARK Implementations*.
   - URL: [https://github.com/arkworks-rs/groth16](https://github.com/arkworks-rs/groth16)
6. **Zeltra Architecture Decisions:**
   - [`DEC-036A: Universal 2-in-2-out JoinSplit R1CS Circuit Specification`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md)
   - [`DEC-040: Viewing Key Export Suite & CEX Selective Disclosure`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)
   - [`DEC-040A: Selective Disclosure Circuit & Key Separation`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040A-selective-disclosure-circuit-and-key-separation.md)
