# DEC-042: Privacy-Preserving HTTP 402 AI Agent Facilitator & Pure ZK-UTXO Micropayment Protocol

**Status:** APPROVED  
**Date:** 2026-10-16  
**Domain:** `zeltra-node` (x402 Facilitator Ingress & Settlement Engine), `zeltra-sdk` (Client AI Agent x402 Codec & Payment Signature Generator), `zeltra-contracts` (Arbitrum Stylus Settlement Verification)  
**Security Level:** CRITICAL (Agentic Machine-to-Machine Financial Solvency, Replay Immunity, Anti-Double-Spend, Privacy Preservation, Zero-Gas-Griefing)  
**Related Decisions:**
- [`DEC-026: Receiver-Enforced Compliance & Zero-Cost Sanctions Filtering (Fail-Closed Preflight)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)
- [`DEC-028: ZK Public Input Domain Hardening & Flat 45 bps Fee Unification`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-028-zk-public-input-domain-hardening-flat-fee-unification-and-compliance-scoping.md)
- [`DEC-031: Relayer Reconciliation, Solvency Observability & Quote Domain Binding`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md)
- [`DEC-032: Merkle Mountain Range Commitment Accumulator`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md)
- [`DEC-034: Dual-Asset Architecture (USDC & WETH Sovereign Commerce Rails)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md)
- [`DEC-035A: Two-Limb Quote Hash Representation and Range Constraints`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md)
- [`DEC-035B: Formal Scope Binding Gadget & Groth16 Statement Integrity`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035B-formal-scope-binding-gadget-and-groth16-statement-integrity.md)
- [`DEC-038: Pure ZK-UTXO Migration & Direct MMR Deposit`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
- [`DEC-038A: Zero-Legacy Purge & Threshold BLS Elimination`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038A-zero-legacy-purge-and-threshold-bls-elimination.md)
- [`DEC-038C: Relayer Sequencer & SDK Direct Deposit`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038C-relayer-sequencer-sdk-direct-deposit-and-cli.md)
- [`DEC-041: In-Browser WebAssembly Groth16 Prover & Disposable Worker`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-041-in-browser-wasm-groth16-prover-and-disposable-worker.md)

---

## 1. Executive Summary & Context

Ekosistem agen otonom (*autonomous AI Agents*) berkembang pesat pada tahun 2024–2026. Untuk melakukan transaksi antar-mesin (*machine-to-machine* / M2M) seperti pembayaran komputasi inferensi LLM, akses API data finansial real-time, atau scraping data berbayar, konsorsium industri melalui **The Linux Foundation x402 Foundation** (didukung oleh Coinbase, Cloudflare, Stripe, AWS, dan Vercel) membakukan protokol **HTTP 402 ("Payment Required")** melalui spesifikasi **x402 v2**, yang juga diadopsi ke dalam **Google Agentic Payments Protocol (AP2)**.

Data transaksi industri mencatat lebih dari **138 juta transaksi x402**, di mana **87%–90% merupakan transaksi mikro (micropayments bernilai $0.0001 hingga $0.10)** untuk inferensi model AI per-token dan data scraping.

### A. Cacat Kritis Standar x402 Eksisting (Ketiadaan Privasi Finansial)
Standar x402 v2 saat ini mengandalkan tanda tangan EVM publik standar seperti **EIP-3009** (`transferWithAuthorization`) atau transfer ERC-20 langsung. Hal ini menimbulkan kelemahan fatal:
1. **Kebocoran Identitas Total AI Agent:** Setiap kali agen membayar satu panggilan API seharga $0.001 USDC, alamat dompet publik agen (`from`), saldo perbendaharaan total, dan seluruh riwayat transaksi terekspos ke vendor API.
2. **Cluster Analisis Perilaku Agen:** Vendor API atau analis on-chain dapat melakukan *graph clustering* untuk memetakan strategi operasional agen, sumber modal, dan relasi bisnis agen dengan pihak ketiga.
3. **Penyalahgunaan Data PII:** Sebagaimana dibuktikan dalam riset keamanan (*"Hardening x402: PII-Safe Agentic Payments"*, arXiv:2604.11430), ketiadaan privasi pada payload x402 membahayakan kerahasiaan korporat yang mempekerjakan AI Agent tersebut.

### B. Keterbatasan Solusi Privasi x402 Generasi Pertama (nativ3ai/x402-privacy-adapter)
Eksperimen awal komunitas privasi (seperti `nativ3ai/x402-privacy-adapter` atau `PrivateX402`) mencoba menambahkan ZK ke x402, namun terbentur batasan struktural:
- *Fixed Denominations:* Hanya mendukung denominasi kaku ($5, $10, $20, $50, $100 USDC), tidak mampu melayani micropayment dinamis ($0.0001 – $0.05).
- *Standard Merkle Tree Insertion Cost:* Biaya gas on-chain tinggi saat memasukkan komitmen deposit baru.
- *Ketiadaan Formal Scope Binding:* Rawan serangan desinkronisasi quote fee eksekusi relayer.

### C. Mismatch Arsitektur Internal Zeltra (Legacy BLS vs Pure ZK-UTXO)
Implementasi awal modul `x402` di Zeltra (`zeltra-sdk/src/x402/` dan `zeltra-node/src/handlers/x402.rs`) dibangun pada era pra-DEC-038 ketika Zeltra masih menggunakan skema *BLS Blind Signature* dengan threshold guardian signer (`alpha_neg_hex`, `hm_hex`, `pk_iss_hex`).

Ketika protokol bermigrasi 100% ke arsitektur **Pure ZK-UTXO** ([`DEC-038`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)), entrypoint kontrak pintar `spend(...)` dan `batchSpend(...)` **dihapus bersih** dari kontrak Stylus (`zeltra-contracts/src/lib.rs`). Modul `x402` tertinggal dan masih berusaha memanggil interface legacy yang sudah punah tersebut, sehingga jika diproses di on-chain akan langsung mengalami **revert mutlak**.

### D. Solusi DEC-042: Modernisasi Penuh ke Pure ZK-UTXO x402 Facilitator
Dokumen ini menetapkan spesifikasi formal **DEC-042**: Modernisasi menyeluruh modul `x402` di Zeltra Protocol menjadi **Privacy-Preserving HTTP 402 Facilitator** berbasis **Pure ZK-UTXO Private Note**:
- **Zero Identity Leakage:** AI Agent membayar menggunakan bukti tanpa-pengetahuan (*Groth16 Zero-Knowledge Proof*) atas kepemilikan private note di Merkle Mountain Range (MMR). Alamat dompet pengirim tidak pernah muncul di blockchain on-chain.
- **Dynamic Micropayments ($0.0001 hingga jutaan USDC/WETH):** Didukung penuh oleh 64-bit integer range constraints dan Two-Limb quote hash ([`DEC-035A/B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md)), tanpa batasan denominasi kaku.
- **Universal Merchant Payout:** Merchant atau penyedia API menerima pembayaran langsung dalam bentuk **USDC atau WETH murni** di alamat EVM publik mereka tanpa perlu menjalankan node ZK atau mengubah infrastruktur akuntansi mereka.
- **Instant Micro-Payment Authorization (< 50 ms):** Facilitator memverifikasi validitas bukti ZK dan keunikan nullifier secara *fail-closed preflight* di off-chain sebelum mengizinkan respon HTTP 200 dilepaskan ke AI Agent.
- **Pembersihan Bersih (Zero-Legacy):** Seluruh DTO, parameter, dan fungsi peninggalan BLS blind signature dimusnahkan 100% dari SDK dan Relayer Node.

---

## 2. Analisis Keamanan & Serangan Terhadap Protokol x402 (2024–2026)

Berdasarkan riset kriptografi terbaru terhadap protokol x402 (*"Five Attacks on x402 Agentic Payment Protocol"*, arXiv:2605.11781v1), arsitektur DEC-042 Zeltra memitigasi seluruh vektor serangan umum:

| Vektor Serangan x402 | Modus Operandi | Mitigasi Kriptografi DEC-042 Zeltra |
|:---|:---|:---|
| **Attack 1: Free-Riding & Signature Recycling** | Agen jahat menggunakan signature EIP-3009 yang sama ke beberapa resource provider sekaligus sebelum tx on-chain ditambang. | **Siloed Nullifier Double-Spend Prevention:** Setiap private note memiliki nullifier deterministik tunggal ($\text{nullifier} = \text{Poseidon}(sk, \rho)$). Node Relayer memeriksa DB lokal (SQLite WAL) dan kontrak on-chain dalam satu transaksi atomik. Replay ditolak seketika (< 2 ms). |
| **Attack 2: Front-Running & Mempool Snooping** | Pihak ketiga mengintip mempool publik, mencuri signature otorisasi bayar, dan mengganti parameter resource URI. | **Semantic Scope Binding (`quote_hash` & `recipient`):** Parameter `recipient` (alamat merchant) dan nilai nominal dikunci mati (*hardcoded*) di dalam 16 Public Inputs Groth16. Pihak ketiga tidak dapat mengubah alamat penerima tanpa membatalkan persamaan pairing $e(A, B)$. |
| **Attack 3: Double-Spend In-Flight Race** | Dua permintaan HTTP konkuren dikirim serentak menggunakan input note yang sama untuk mengelabui resource server. | **Transactional Database Lock (`TransactionBehavior::Immediate`):** Relayer mengunci entri nullifier di tabel `spend_queue` sebelum melepaskan respon verifikasi HTTP 402 Facilitator. |
| **Attack 4: Gas-Griefing & Denial of Service** | Agen jahat mengirimkan payload bukti ZK rusak atau skalar non-kanonikal untuk menguras kuota gas relayer. | **Fail-Closed Local Groth16 Preflight Check (DEC-026 C-01):** Relayer memverifikasi bukti Groth16 secara lokal di memori CPU sebelum melakukan interaksi RPC atau penyiaran transaksi ke Arbitrum Stylus. Bukti tidak valid langsung dibuang tanpa biaya gas. |
| **Attack 5: Merchant Payout Underpayment** | Agen membayar nilai kotor tanpa memperhitungkan potongan fee protokol on-chain (45 bps), menyebabkan merchant menerima dana lebih sedikit dari harga resource. | **Exact Payout Conservation Invariant:** R1CS sirkuit dan kontrak mewajibkan: $\text{Input Note} = \text{Merchant Amount} + \text{Protocol Fee} (45\text{ bps}) + \text{Execution Fee} + \text{Change Note}$. Merchant dijamin menerima 100% dari harga yang diminta. |

---

## 3. Alur Kerja Protokol (Protocol Handshake Flow)

```
┌──────────────┐                 ┌──────────────────────┐                 ┌────────────────────────┐                 ┌───────────────────────┐
│   AI Agent   │                 │   Resource Server    │                 │   Zeltra Facilitator   │                 │ Arbitrum Stylus WASM  │
│ (Client SDK) │                 │      (API Vendor)    │                 │     (Relayer Node)     │                 │   (Zeltra Contract)   │
└──────┬───────┘                 └──────────┬───────────┘                 └───────────┬────────────┘                 └───────────┬───────────┘
       │                                    │                                         │                                          │
       │ 1. GET /v1/market-data             │                                         │                                          │
       ├───────────────────────────────────►│                                         │                                          │
       │                                    │                                         │                                          │
       │ 2. HTTP 402 Payment Required       │                                         │                                          │
       │    Header: PAYMENT-REQUIRED        │                                         │                                          │
       │    (Base64 JSON: price, asset,     │                                         │                                          │
       │     payTo, network: eip155:42161)  │                                         │                                          │
       │◄───────────────────────────────────┤                                         │                                          │
       │                                                                              │                                          │
       │ 3. Client SDK Note Selection & ZK Proof Generation                           │                                          │
       │    • Pilih Note UTXO via Knapsack                                            │                                          │
       │    • Hitung Fee: Flat 45 bps + Gas Quote                                     │                                          │
       │    • Generate Groth16 Proof (PrivateNoteCircuit 16 PIs)                       │                                          │
       │    • Encode X402PaymentSignature (Base64 JSON)                               │                                          │
       │                                                                              │                                          │
       │ 4. GET /v1/market-data (Retried)   │                                         │                                          │
       │    Header: PAYMENT-SIGNATURE       │                                         │                                          │
       ├───────────────────────────────────►│                                         │                                          │
       │                                    │                                         │                                          │
       │                                    │ 5. POST /api/x402/verify                │                                          │
       │                                    │    Body: { payment_signature_b64, ... } │                                          │
       │                                    ├────────────────────────────────────────►│                                          │
       │                                    │                                         │                                          │
       │                                    │                                         │ 6. Fast Verification Pipeline            │
       │                                    │                                         │    • Base64 & Schema parsing             │
       │                                    │                                         │    • Nullifier uniqueness in DB & onchain│
       │                                    │                                         │    • Recipient & Amount match check      │
       │                                    │                                         │    • Fail-Closed Groth16 Preflight Check │
       │                                    │                                         │    • Persist to spend_queue (Atomic Lock)│
       │                                    │                                         │                                          │
       │                                    │ 7. HTTP 200 { success: true, ... }      │                                          │
       │                                    │◄────────────────────────────────────────┤                                          │
       │                                    │                                         │                                          │
       │ 8. HTTP 200 OK (Data Delivery)     │                                         │ 8. Async Settlement Broadcast            │
       │    Body: { data: "market_data" }   │                                         │    evm_client.broadcast_spend_private_note_transaction
       │◄───────────────────────────────────┤                                         ├─────────────────────────────────────────►│
       │                                    │                                         │                                          │ 9. EIP-2537 MSM+Pairing
       │                                    │                                         │                                          │    Burn Nullifier
       │                                    │                                         │                                          │    Transfer USDC to Vendor
       │                                    │                                         │                                          │    Append Change Note to MMR
       │                                    │                                         │                                          │◄─────────────────────────┤
```

---

## 4. Spesifikasi Struktur Data & Skema Wire-Format

### A. Wire-Format Standar x402 v2
Mengikuti standar Coinbase x402 v2, header HTTP menggunakan format Base64 dari payload JSON UTF-8.

#### 1. Header `PAYMENT-REQUIRED` (Disajikan oleh Server):
```json
{
  "accepts": [
    {
      "scheme": "exact",
      "network": "eip155:42161",
      "price": {
        "amount": "1000",
        "asset": "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
        "extra": {
          "name": "USDC",
          "version": "1",
          "decimals": 6
        }
      },
      "payTo": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e"
    }
  ],
  "description": "High-Frequency AI Market Sentiment Feed",
  "mimeType": "application/json"
}
```

#### 2. Header `PAYMENT-SIGNATURE` (Dihasilkan oleh AI Agent via Zeltra SDK):
Payload `payment` di dalam `X402PaymentSignature` memuat seluruh data pembuktian ZK-UTXO `PrivateNoteSpendRequest` yang kanonikal:
```json
{
  "x402Version": 2,
  "scheme": "exact",
  "network": "eip155:42161",
  "payment": {
    "asset_id": 1,
    "note_root": "0x12ab...",
    "leaf_count": 142,
    "note_epoch_id": 1,
    "input_nullifier": "0x34cd...",
    "output_commitment": "0x56ef...",
    "recipient": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
    "merchant_amount": 1000,
    "protocol_fee": 5,
    "execution_fee": 15,
    "max_execution_fee": 30,
    "quote_hash": "0x78ab...",
    "proof_a_neg": "0x0123... (128 bytes EVM hex)",
    "proof_b": "0x0456... (256 bytes EVM hex)",
    "proof_c": "0x0789... (128 bytes EVM hex)",
    "public_inputs": [
      "0x... (32 bytes scalar hex)",
      "..."
    ],
    "change_encrypted_memo": "0x9abc... (96 bytes AEAD memo)"
  }
}
```

---

## 5. Implementasi Rust SDK (`zeltra-sdk/src/x402/`)

### A. Data Structures (`types.rs`)
Struktur data lama yang memuat komponen BLS (`alpha_neg_hex`, `hm_hex`, `pk_iss_hex`) dihapus total dan digantikan oleh `X402ZeltraPayment`:

```rust
/// Payload pembayaran anonim Zeltra ZK-UTXO yang disematkan di dalam header PAYMENT-SIGNATURE
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct X402ZeltraPayment {
    #[serde(default)]
    pub asset_id: Option<u8>,
    pub note_root: String,
    #[serde(default)]
    pub leaf_count: Option<u64>,
    #[serde(default)]
    pub note_epoch_id: Option<u32>,
    pub input_nullifier: String,
    #[serde(default)]
    pub output_commitment: Option<String>,
    pub recipient: String,
    pub merchant_amount: u64,
    #[serde(default)]
    pub protocol_fee: u64,
    #[serde(default)]
    pub execution_fee: u64,
    #[serde(default)]
    pub max_execution_fee: u64,
    #[serde(default)]
    pub quote_hash: Option<String>,
    #[serde(default)]
    pub quote_signature: Option<String>,
    #[serde(default)]
    pub quote_id: Option<String>,
    #[serde(default)]
    pub expiry: Option<u64>,
    pub proof_a_neg: String,
    pub proof_b: String,
    pub proof_c: String,
    pub public_inputs: Vec<String>,
    #[serde(default)]
    pub change_encrypted_memo: Option<String>,
}

/// Struktur utama PAYMENT-SIGNATURE header x402 v2
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402PaymentSignature {
    #[serde(rename = "x402Version")]
    pub x402_version: u32,
    pub scheme: String,
    pub network: String,
    pub payment: X402ZeltraPayment,
}
```

### B. Codec Helpers (`codec.rs`)
Fungsi builder diselaraskan untuk mengonversi bukti ZK secara langsung:
```rust
/// Membangun X402PaymentSignature dari bukti ZK Private Note yang valid
pub fn build_x402_zk_payment_signature(
    network: &str,
    scheme: &str,
    payment: X402ZeltraPayment,
) -> X402PaymentSignature {
    X402PaymentSignature {
        x402_version: 2,
        scheme: scheme.to_string(),
        network: network.to_string(),
        payment,
    }
}
```

---

## 6. Pipeline Verifikasi Facilitator Relayer (`zeltra-node`)

Endpoint `POST /api/x402/verify` diimplementasikan dengan 7 langkah verifikasi defensif yang ketat:

1. **Base64 & JSON Decode:**
   Mendekode nilai header `PAYMENT-SIGNATURE` dari string base64 menjadi struct `X402PaymentSignatureInner`. Jika gagal, segera respon `400 Bad Request` dengan deskripsi kegagalan parsing.
2. **Protokol Guard (`x402Version == 2`):**
   Memastikan versi protokol adalah 2. Versi lain langsung ditolak.
3. **Pemeriksaan Skalar Kanonikal ($< r$):**
   Memastikan seluruh skalar di dalam `public_inputs`, `input_nullifier`, dan `note_root` adalah elemen lapangan BLS12-381 yang sah (`scalar < MODULUS`).
4. **Pencegahan Double-Spend Dua Lapis:**
   - Layer 1: Query database persistent SQLCipher: `state.db.is_nullifier_spent(&nullifier)`.
   - Layer 2: Jika terkonfigurasi EVM client, lakukan verifikasi view call on-chain: `contract.is_nullifier_spent(nullifier)`.
5. **Validasi Parameter Pembayaran:**
   - Memastikan `payment.merchant_amount >= required_amount`.
   - Memastikan alamat `payment.recipient` valid dan sama dengan alamat merchant (`pay_to`) atau konfigurasi facilitator.
6. **Fail-Closed Groth16 Preflight Verification (DEC-026 C-01):**
   Relayer memverifikasi bukti Groth16 secara lokal di CPU menggunakan verifying key sirkuit `PrivateNoteCircuit`. Ini menjamin transaksi tidak akan pernah revert di blockchain saat disiarkan oleh relayer.
7. **Enqueue ke Mempool Persistence (`SpendRequest`):**
   Mengonversi `X402ZeltraPayment` ke `SpendRequest` dengan tipe `spend_type = "private_note"` dan mengeksekusi `state.db.enqueue_spend(&spend_req)`. Transaksi settlement on-chain akan disiarkan secara asinkronus oleh background worker menggunakan `broadcast_spend_private_note_transaction`.

---

## 7. Pembersihan Kode Usang (Zero-Legacy Purge)

Sejalan dengan prinsip dasar Zeltra (**"Zero AI-Slop & Zero Zombie Code"**), modernisasi DEC-042 mencakup penghapusan kode mati peninggalan BLS blind signature:

1. **`zeltra-node/src/evm_client.rs`:**
   - Menghapus definisi `function spend(...)` dan `function batchSpend(...)` dari blok makro `sol! { ... }`.
   - Menghapus implementasi fungsi mati `broadcast_spend_transaction(...)`.
2. **`zeltra-node/src/handlers/spend.rs`:**
   - Menghapus pembentukan struktur usang `batch_item(...)` dan fungsi `process_same_chain_batch(...)` yang menargetkan `batchSpendCall`.
3. **`zeltra-sdk/src/x402/`:**
   - Menghapus seluruh referensi fungsi `build_zeltra_payment_signature` yang menggunakan parameter `alpha_neg_hex`, `hm_hex`, `pk_iss_hex`.

---

## 8. Invariants Matematis & Finansial

Pelanggaran terhadap salah satu invariant di bawah ini dikategorikan sebagai **Critical Security Bug**:

1. **INVARIANT 1 (Absolute Buyer Privacy):**
   Alamat akun asal, public key pengirim, dan nilai input UTXO **dilarang keras** terekspos dalam header `PAYMENT-SIGNATURE` maupun transaksi broadcast on-chain. Hanya `input_nullifier`, `output_commitment` (change note), dan `recipient` yang muncul ke publik.
2. **INVARIANT 2 (Universal Exact Value Conservation):**
   $$\text{Input Note Value} = \text{Merchant Amount} + \text{Protocol Fee (45 bps)} + \text{Execution Fee} + \text{Change Note Value}$$
   Selisih saldo sekecil 1 unit basis (1 micro-USDC atau 1 Wei) wajib membatalkan bukti ZK melalui 64-bit range constraints.
3. **INVARIANT 3 (Single-Spend Nullifier Siloing):**
   Satu nullifier hanya dapat digunakan tepat satu kali untuk menyelesaikan satu pembayaran x402. Upaya penyerahan nullifier yang sama untuk kedua kalinya wajib ditolak seketika pada layer database tanpa memicu penyiaran on-chain.
4. **INVARIANT 4 (Fail-Closed Preflight Protection):**
   Relayer **dilarang menyiarkan** transaksi settlement x402 ke jaringan Arbitrum jika bukti Groth16 belum diverifikasi valid secara lokal di CPU off-chain.
5. **INVARIANT 5 (Standard x402 v2 Wire Compatibility):**
   Header `PAYMENT-REQUIRED` dan `PAYMENT-SIGNATURE` wajib mematuhi spesifikasi standar Base64 JSON x402 v2, menjamin interoperabilitas instan dengan pustaka agen AI standar (seperti LangChain, AutoGen, CrewAI, atau Coinbase AgentKit).

---

## 9. Matriks Pengujian Formal & Soundness

Rasio pengujian negatif wajib $\ge 2\times$ pengujian positif:

### Positive Tests:
1. `test_x402_zk_payment_signature_encode_decode_roundtrip`:
   Verifikasi serialisasi dan deserialisasi payload `X402PaymentSignature` berbasis ZK-UTXO ke format Base64 JSON.
2. `test_x402_facilitator_verifies_valid_groth16_payment`:
   Simulasi penuh pengiriman bukti Groth16 yang valid ke endpoint `/api/x402/verify`, menghasilkan status `success: true` dan nomor antrean settlement `queue_id`.
3. `test_x402_merchant_receives_exact_price`:
   Memverifikasi bahwa nilai bersih yang diterima oleh merchant sama persis dengan `price.amount` yang tercantum di header `PAYMENT-REQUIRED`.

### Negative Tests (Boundary & Anti-Griefing):
1. `test_x402_rejects_unsupported_version`:
   Payload dengan `x402Version != 2` langsung ditolak dengan pesan error terstruktur.
2. `test_x402_rejects_non_canonical_scalar_in_proof`:
   Skalar di dalam `public_inputs` atau `nullifier` bernilai $\ge r$ ditolak seketika pada tahap preflight.
3. `test_x402_rejects_double_spend_nullifier`:
   Pengiriman bukti dengan nullifier yang sudah tercatat di database ditolak dengan error `Double-spending detected`.
4. `test_x402_rejects_invalid_groth16_proof`:
   Pengiriman bukti dengan titik kurva palsu/acak ditolak oleh preflight verification off-chain tanpa menimbulkan biaya gas relayer.
5. `test_x402_rejects_recipient_mismatch`:
   Pengiriman bukti di mana field `recipient` berbeda dari alamat merchant yang diminta ditolak dengan error validasi.

---

## 10. Referensi Akademik & Standar Industri

1. **Coinbase x402 Specification v2:**
   - *HTTP 402 Payment Required for AI Agents & Machine-to-Machine Commerce*.
   - URL: [https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md](https://github.com/coinbase/x402/blob/main/specs/x402-specification-v2.md)
2. **The Linux Foundation x402 Foundation (2025–2026):**
   - *Cross-Industry Open Standard for Agentic Payments (AWS, Cloudflare, Stripe, American Express, Coinbase)*.
3. **Google Agentic Payments Protocol (AP2) Integration:**
   - *Autonomous Stablecoin Settlements across AI Agent Frameworks (LangChain, CrewAI, AutoGen)*.
4. **nativ3ai x402 Privacy Adapter:**
   - *Buyer-Side Privacy Adapter for x402 via ZK Note Spends (Fixed Denominations)*.
   - URL: [https://github.com/nativ3ai/x402-privacy-adapter](https://github.com/nativ3ai/x402-privacy-adapter)
5. **Kriptografi & Analisis Serangan x402 (arXiv:2605.11781v1):**
   - *Five Attacks on x402 Agentic Payment Protocol: Free-Riding, Double-Spending, and Logic Flaws*.
   - URL: [https://arxiv.org/abs/2605.11781](https://arxiv.org/abs/2605.11781)
6. **Privasi Agen & Analisis PII (arXiv:2604.11430):**
   - *Hardening x402: PII-Safe Agentic Payments and Transaction Graph Obfuscation*.
   - URL: [https://www.arxiv.org/pdf/2604.11430](https://www.arxiv.org/pdf/2604.11430)
7. **CAIP-2 Blockchain Identification:**
   - *Chain Agnostic Identifier for Arbitrum One (`eip155:42161`) and Sepolia (`eip155:421614`)*.
   - URL: [https://github.com/ChainAgnostic/CAIPs/blob/master/CAIPs/caip-2.md](https://github.com/ChainAgnostic/CAIPs/blob/master/CAIPs/caip-2.md)
8. **Zeltra Architecture Decisions:**
   - [`DEC-026: Receiver-Enforced Compliance & Zero-Cost Sanctions Filtering`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)
   - [`DEC-035A/B: Formal Scope Binding Gadget & Two-Limb Quote Hash`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md)
   - [`DEC-038: Pure ZK-UTXO Migration & Direct MMR Deposit`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
   - [`DEC-041: In-Browser WebAssembly Groth16 Prover & Disposable Worker`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-041-in-browser-wasm-groth16-prover-and-disposable-worker.md)
