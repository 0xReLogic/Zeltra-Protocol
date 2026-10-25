# DEC-040: Viewing Key Export Suite, B2B Audit Trail & CEX Selective Disclosure Compliance Protocol

**Status:** APPROVED  
**Date:** 2026-10-15  
**Domain:** `zeltra-sdk` (Export & Verification Engine), `zeltra-cli` (Compliance Tooling), `zeltra-core` (Selective Disclosure Circuits & Key Derivation)  
**Security Level:** CRITICAL (Financial Privacy, Anti-Impersonation, Zero-Tech-Debt & Anti-Deanonymization)  
**Related Decisions:**
- [`DEC-026: Receiver-Enforced Compliance & Zero-Cost Sanctions Filtering`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)
- [`DEC-032: Merkle Mountain Range Commitment Accumulator`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-032-merkle-mountain-range-commitment-accumulator.md)
- [`DEC-034: Dual-Asset Architecture (USDC Commerce & WETH Sovereign)`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md)
- [`DEC-034A: Canonical Field Bit-Packing and Intra-Asset Circuit Constraints`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md)
- [`DEC-035A: Two-Limb Quote Hash Representation and Range Constraints`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md)
- [`DEC-037: Note Encryption Memo Protocol & Viewing Key Recovery`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-037-note-encryption-memo-and-viewing-key-recovery.md)
- [`DEC-038: Pure ZK-UTXO Migration & Direct MMR Deposit`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
- [`DEC-039: Memo Integrity Gap Mitigation`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-039-MEMO_INTEGRITY_GAP.md)

---

## 1. Executive Summary & Context

Zeltra Protocol adalah infrastruktur rel pembayaran privat universal (*"The Stripe of Web3 with Absolute Privacy"*). Dalam model kas nyata (*Private Cash*), privasi transaksi adalah nilai mutlak (*default privacy* 10/10). Namun, adopsi skala besar oleh entitas korporat (B2B), merchant global, institusi, dan AI Agent menuntut kemampuan **selective disclosure (keterbukaan selektif)** untuk:
1. **Kepatuhan Pajak & Audit Finansial B2B:** Pelaporan SPT Tahunan, PPh/PPN badan usaha, dan audit independen (Big-4 / IFRS / SOC 2) atas arus kas masuk/keluar privat.
2. **Deposit Bursa Terpusat (CEX Source of Funds):** Menembus pembekuan deposit di CEX (seperti Binance, Coinbase, Kraken, OKX) akibat deteksi interaksi dengan sistem privasi on-chain, dengan membuktikan keaslian sumber dana tanpa membuka seluruh dompet.
3. **Mantra Kepatuhan DEC-026:** *"Permissionless Privacy, Permissioned Acceptance"*.

Tantangannya: **Fitur kepatuhan audit dan ekspor viewing key di ekosistem Web3 sering kali menjadi pintu masuk eksploitasi peretasan dan kebocoran privasi fatal** jika dirancang secara ceroboh.

Dokumen ini mendefinisikan arsitektur formal **Viewing Key Export Suite** yang menjamin:
- **Tingkat 1 (Selective Note Disclosure):** Membuktikan kepemilikan dan sumber dana atas satu note tertentu ke CEX tanpa membocorkan note lainnya atau spending key.
- **Tingkat 2 (Time-Windowed B2B Audit Report):** Mengekspor laporan pembukuan periodik (JSON/CSV) dengan rekonsiliasi persamaan akuntansi UTXO seimbang (*balanced double-entry*).
- **Tingkat 3 (Full Viewing Key Delegation - Zero Tech Debt):** Pendelegasian kunci baca (*read-only*) permanen via pemisahan kunci `spending_key`, `nullifier_deriving_key`, dan `incoming_viewing_key` sehingga auditor internal dapat memantau arus kas masuk dan keluar tanpa kuasa membelanjakan dana.
- **Kebal Pemalsuan & Impersonasi:** Mengikat bukti secara kriptografis ke identitas penerima verifikasi (*Verifier-Bound Challenge*) dan transaksi pencairan on-chain untuk mencegah pencurian atau penggunaan ulang laporan oleh pihak ketiga.

---

## 2. Post-Mortem & Security Archaeology (2020–2026)

Sebelum merancang DEC-040, kami membedah kegagalan sistem kepatuhan privasi masa lalu:

### A. Tornado Cash Compliance Tool Note Leak (Februari 2020)
* **Vektor Serangan:** Tornado Cash merilis alat kepatuhan web di mana pengguna diminta memasukkan *raw secret note*. Fitur *Share URL* menyematkan rahasia note pada parameter URL query (`?note=...`). Ketika dibuka, browser secara otomatis mengirimkan rahasia note dalam HTTP header `Referer` ke server pihak ketiga dan analitik.
* **Dampak:** 98 note privat bocor ke log jaringan; penyerang dapat langsung mencuri dana pada note yang belum di-spend.
* **Mitigasi Zeltra:** **Zero-Remote Exposure.** Seluruh modul ekspor dan verifikasi DEC-040 berjalan 100% lokal di mesin klien (`zeltra-sdk` / `zeltra-cli`). Spending Key (`sk`) dan seed wallet **dilarang keras** pernah disentuh atau disertakan dalam struktur ekspor.

### B. Zcash ZIP-310 Over-Disclosure Threat (2019–2024)
* **Vektor Analisis:** Sesuai telaah *ZIP-310 (Security Properties of Sapling Viewing Keys)*, ketika pengguna memberikan `Incoming Viewing Key (ivk)` kepada auditor untuk membuktikan satu transaksi, auditor tersebut secara permanen memperoleh kemampuan memata-matai **seluruh transaksi masuk di masa lalu dan seluruh transaksi di masa depan**.
* **Mitigasi Zeltra:** **Granular Privilege Separation.** Zeltra membedakan secara tegas antara *Single-Note Disclosure* (hanya membuktikan 1 note spesifik tanpa membocorkan IVK) dengan *Account-Level Audit Delegation*.

### C. The "Stolen Proof" / Audit Impersonation Attack
* **Vektor Serangan:** Alice mengekspor laporan bukti bahwa dia menerima $50.000 USDC di Zeltra dan mengirimkannya ke auditor atau mengunggahnya. Penyerang (Mallory) menyalin berkas JSON tersebut dan mengirimkannya ke Binance dengan klaim: *"Ini bukti bahwa deposit $50.000 USDC saya bersih dan berasal dari Zeltra."*
* **Dampak:** Penyerang dapat mencuci dana haram dengan menunggangi bukti kepatuhan milik orang lain.
* **Mitigasi Zeltra:** **Cryptographic Verifier Binding.** Setiap bukti selektif diikat dengan tanda tangan ZK/challenge ke *Verifier ID* (misal: `BINANCE_KYC_DESK` + `AccountUID` + `Timestamp` + `Nonce` + `SpentTxHash` + `Nullifier` + `PayoutRecipient`). Verifikasi akan langsung gagal jika diajukan oleh atau untuk entitas yang berbeda.

### D. The "Ghost Asset" / Spoofed Spend Fraud
* **Vektor Serangan:** Pengguna nakal memberikan plaintext komitmen $C$ yang valid, lalu mengklaim note telah di-spend ke Binance dengan menempelkan nullifier acak atau mencuri nullifier dari transaksi orang lain di explorer blockchain.
* **Mitigasi Zeltra:** Nullifier dibuktikan secara matematis terkait dengan note melalui sirkuit NIZK pembuktian kepemilikan (`SelectiveDisclosureCircuit`) dan jalur MMR lengkap diverifikasi langsung terhadap state on-chain Arbitrum Stylus.

### E. Dual-Asset Desynchronization (DEC-034 Context)
* **Vektor Masalah:** Kesalahan interpretasi satuan antara WETH (`asset_id = 0`, satuan Gwei $10^{-9}$) dan USDC (`asset_id = 1`, satuan mikro $10^{-6}$) yang dapat menyebabkan distorsi laporan pajak atau neraca audit hingga miliaran rupiah.
* **Mitigasi Zeltra:** Skema ekspor mewajibkan pencantuman eksplisit `asset_id`, `symbol`, `circuit_unit_name`, `circuit_decimals`, `erc20_decimals`, nilai raw circuit value, nilai skalar `value_packed`, dan nilai terformat human-readable.

---

## 3. Tingkatan Kepatuhan (Granular Compliance Hierarchy)

DEC-040 menetapkan 3 tingkatan ekspor berdasarkan prinsip hak akses terendah (*Principle of Least Privilege*):

```
┌────────────────────────────────────────────────────────────────────────┐
│                        ZELTRA COMPLIANCE HIERARCHY                     │
├────────────────────────────────────────────────────────────────────────┤
│ Tier 1: Single-Note CEX Proof (Selective Disclosure)                   │
│   ├── Target: CEX KYC/AML Officers (Binance, Coinbase, Kraken, OKX)    │
│   ├── Scope: HANYA 1 Note Spesifik yang didepositkan                   │
│   └── Privasi: 100% Note lain di wallet tetap anonim & terisolasi      │
├────────────────────────────────────────────────────────────────────────┤
│ Tier 2: Time-Windowed B2B Audit Trail (JSON / CSV)                     │
│   ├── Target: Auditor Kantor Akuntan Publik (KAP), Pelaporan SPT Pajak │
│   ├── Scope: Transaksi masuk/keluar dalam rentang tanggal tertentu     │
│   └── Format: Jurnal Kas Standar Akuntansi Finansial (Double-Entry)   │
├────────────────────────────────────────────────────────────────────────┤
│ Tier 3: Read-Only Full Viewing Key Delegation (FVK)                    │
│   ├── Target: Departemen Finansial Internal / Custody Compliance       │
│   ├── Scope: Hak audit pasif membaca seluruh arus kas masuk & keluar   │
│   └── Keamanan: Zero Spend Authority (Pemisahan sk vs nk vs ivk)       │
└────────────────────────────────────────────────────────────────────────┘
```

### Arsitektur Kunci Tingkat 3 (Zero Tech Debt — Key Separation):
Untuk menghilangkan *technical debt* dan memungkinkan audit arus kas keluar (*outflow*) tanpa membocorkan hak belanja (*spend authority*):
1. **`spending_key` ($sk \in \text{Fr}$):** Kunci privat master (hanya disimpan di dompet klien, tidak pernah didelegasikan).
2. **`nullifier_deriving_key` ($nk = \text{Poseidon\_W3}(sk, 0, \text{DOMAIN\_NULLIFIER\_KEY})$):**
   Kunci untuk menurunkan nullifier: $\text{nf} = \text{Poseidon\_W5}([nk, \rho, \text{epoch\_id}, 0])$. Pemegang $nk$ dapat memeriksa apakah note miliknya sudah di-spend di on-chain, tetapi **tidak bisa** membuat spending proof Groth16.
3. **`incoming_viewing_key` ($ivk \in \mathbb{R}^{32}$):** Kunci Diffie-Hellman untuk mendekripsi memo note masuk dari event log on-chain (DEC-037).
4. **`outcoming_viewing_key` ($ovk \in \mathbb{R}^{32}$):** Kunci untuk mendekripsi memo note kembalian (*change note*) dan riwayat pembayaran keluar.
5. **`FullViewingKey` ($fvk = (\text{owner\_key}, ivk, ovk, nk)$):** Paket delegasi audit lengkap yang dapat diberikan kepada auditor internal:
   - Dapat mendeteksi dan mendekripsi semua note masuk (via $ivk$).
   - Dapat mendeteksi dan mendekripsi semua note keluar / change (via $ovk$).
   - Dapat mendeteksi kapan setiap note dibelanjakan (via $nk$).
   - **TIDAK BISA** membelanjakan atau mencuri dana satu sen pun (karena tidak memiliki $sk$).

---

## 4. Protokol Kriptografi Single-Note Selective Disclosure (Tier 1)

Untuk membuktikan sumber dana (*Source of Funds*) atas deposit ke CEX atau pembayaran ke merchant tanpa membocorkan `spending_key` maupun note lain di dalam dompet:

### A. Sinkronisasi Komitmen Kanonikal (`DEC-034A` Bit-Packing Compliant)
Sesuai [`DEC-034A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md), setiap Private Note $N$ memiliki parameter $(v, \text{asset\_id}, \text{owner\_key}, \rho, r)$ di mana:
1. **Bit-Packed Value ($I_0$):**
   $$\text{value\_packed} = \text{Fr::from}(v) + \text{Fr::from}(\text{asset\_id}) \cdot 2^{64}, \quad v \in [0, 2^{64}-1], \ \text{asset\_id} \in \{0, 1\}$$
2. **Derivasi Owner Key ($I_1$) & Nullifier:**
   $$\text{owner\_key} = \text{Fr::from\_be\_bytes\_mod\_order}(\&pk_{\text{X25519}})$$
   $$\text{nf} = \text{Poseidon\_W5}([nk, \rho, \text{epoch\_id}, 0]; \text{DOMAIN\_NULLIFIER})$$
3. **Komitmen Leaf Poseidon-W5:**
   $$C = \text{Poseidon\_W5}([\text{value\_packed}, \text{owner\_key}, \rho, r]; \text{DOMAIN\_NOTE\_COMMITMENT})$$

### B. Komponen Paket Pembuktian $\Pi_{\text{CEX}}$
Karena `owner_key` dan `nf` diturunkan melalui fungsi hash aljabar Poseidon (bukan kurva eliptik Schnorr standar), pembuktian kepemilikan note dan validitas nullifier tanpa mengekspos `spending_key` dilakukan melalui **Lightweight Disclosure Circuit (`SelectiveDisclosureCircuit` - Groth16)**:

Paket $\Pi_{\text{CEX}}$ terdiri dari:
1. **Plaintext Note Terverifikasi:** $(v, \text{asset\_id}, \text{owner\_key}, \rho, r)$.
2. **Bukti Keberadaan MMR Lengkap (`MmrInclusionProof` - `DEC-032`):**
   - `leaf_index`: Indeks daun di MMR.
   - `leaf_count`: Total daun aktif saat bukti dibuat.
   - `peak_index`: Indeks puncak tempat sub-tree daun berada.
   - `sibling_path`: Array hash sibling (`Vec<[u8; 32]>`) dari daun menuju puncak sub-tree.
   - `mmr_peaks`: Array seluruh puncak MMR aktif (`Vec<[u8; 32]>`) untuk verifikasi *bagging* ke `mmr_root`.
   - `mmr_root`: Root komitmen on-chain.
3. **Verifier Binding Challenge Hash (`challenge_hash`):**
   Untuk mencegah *Stolen Proof Attack* dan mengikat bukti ke transaksi pencairan on-chain tertentu:
   $$\text{Challenge} = \text{Blake2s-256}(\text{"ZELTRA\_CEX\_DISCLOSURE\_V1"} \parallel \text{VerifierID} \parallel \text{AccountUID} \parallel \text{Timestamp} \parallel \text{ExpiresAt} \parallel \text{Nonce} \parallel C \parallel \text{nf} \parallel \text{SpentTxHash} \parallel \text{PayoutRecipient})$$
   Hash 256-bit ini dipecah menjadi 2 elemen lapangan 128-bit (`challenge_hi`, `challenge_lo`) sesuai standar [`DEC-035A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md).
4. **Zero-Knowledge Ownership & Nullifier Proof ($\pi_{\text{disclosure}}$):**
   Bukti Groth16 super-ringan (~450 constraints, waktu pembuktian $< 15\text{ ms}$) dengan:
   - **Private Witness:** `spending_key` ($sk$).
   - **Public Inputs (5-PI):** `[owner_key, rho, nullifier, challenge_hi, challenge_lo]`.
   - **R1CS Constraints:**
     1. $nk == \text{Poseidon\_W3}(sk, 0, \text{DOMAIN\_NULLIFIER\_KEY})$
     2. $\text{nullifier} == \text{Poseidon\_W5}([nk, \rho, \text{epoch\_id}, 0]; \text{DOMAIN\_NULLIFIER})$
     3. Mengikat `challenge_hi` dan `challenge_lo` sebagai input publik agar bukti Groth16 tidak dapat dimodifikasi atau digunakan ulang oleh pihak ketiga (*non-malleable challenge binding*).

### C. Algoritma Verifikasi Deterministik (`verify_cex_selective_proof`)
Verifikator (staf kepatuhan CEX / CLI) menjalankan 5 langkah verifikasi secara *fail-closed*:
1. **Validasi Kanonikal & Re-komputasi Komitmen (`DEC-034A`):**
   - Pastikan $\text{asset\_id} \in \{0, 1\}$ dan $v \le \text{u64::MAX}$.
   - Hitung $\text{value\_packed} = \text{Fr::from}(v) + \text{Fr::from}(\text{asset\_id}) \cdot 2^{64}$.
   - Hitung $C' = \text{Poseidon\_W5}([\text{value\_packed}, \text{owner\_key}, \rho, r]; \text{DOMAIN\_NOTE\_COMMITMENT})$.
   - Wajib memenuhi $C' == C$.
2. **Verifikasi Jalur MMR & On-Chain Root (`DEC-032`):**
   - Hitung puncak sub-tree dari `C` dan `sibling_path` berdasarkan `leaf_index`.
   - Lakukan *peak bagging* terhadap `mmr_peaks` untuk mendapatkan `computed_root`.
   - Pastikan `computed_root == mmr_root` dan panggil RPC kontrak Stylus `is_accepted_note_root(mmr_root)` untuk memastikan root tersebut sah di blockchain Arbitrum.
3. **Verifikasi Challenge & Masa Berlaku:**
   - Pastikan `expected_verifier_id == verifier_binding.verifier_id`.
   - Pastikan waktu saat ini $\le \text{expires\_at}$ (maksimal 24 jam sejak `generated_at`).
   - Hitung ulang $\text{Challenge}$ dan dekomposisi 128-bit `challenge_hi`, `challenge_lo`.
4. **Verifikasi Bukti Kepemilikan & Nullifier ($\pi_{\text{disclosure}}$):**
   - Verifikasi bukti Groth16 $\pi_{\text{disclosure}}$ terhadap Public Inputs `[owner_key, rho, nullifier, challenge_hi, challenge_lo]`. Ini membuktikan secara matematis bahwa pembuat laporan adalah pemilik sah `spending_key` dari komitmen $C$ dan bahwa `nullifier` tersebut 100% asli milik komitmen $C$ (bukan nullifier curian).
5. **Verifikasi Status On-Chain Nullifier & Tx Pencairan:**
   - Query RPC kontrak Stylus `is_nullifier_spent(nullifier)` dan pastikan event `Spend` pada `spent_tx_hash` memuat `nullifier` tersebut serta mentransfer aset `asset_id` ke alamat tujuan yang diklaim.

---

## 5. Spesifikasi Skema Data Ekspor (JSON & CSV)

### A. Skema JSON Single-Note CEX Proof (`zeltra_cex_proof_v1.json`)
```json
{
  "schema_version": "zeltra.compliance.cex.v1",
  "generated_at": 1776290400,
  "verifier_binding": {
    "verifier_id": "BINANCE_KYC_DESK",
    "account_uid": "USER-98124712",
    "nonce": "a7f3e829bc14d0219901ab23456789cd",
    "expires_at": 1776376800,
    "challenge_hash": "0x8f12c4...99a1"
  },
  "asset": {
    "asset_id": 1,
    "symbol": "USDC",
    "circuit_unit_name": "MICRO_USDC",
    "circuit_decimals": 6,
    "erc20_decimals": 6,
    "raw_circuit_value": "50000000000",
    "value_packed_scalar": "0x010000000ba43b7400",
    "human_value": "50000.000000"
  },
  "note_cryptography": {
    "commitment": "0x12a9c...77f",
    "owner_key": "0x28af...110",
    "rho": "0x098bf...31a",
    "randomness": "0x011ea...89c",
    "nullifier": "0x33bc...901",
    "nullifier_status": "SPENT",
    "spent_tx_hash": "0x44ca...990",
    "payout_recipient": "0xBinanceUserDepositAddress..."
  },
  "mmr_proof": {
    "leaf_index": 1420,
    "leaf_count": 2048,
    "peak_index": 0,
    "sibling_path": [
      "0x11aa...22bb",
      "0x33cc...44dd"
    ],
    "mmr_peaks": [
      "0x89ee...451"
    ],
    "mmr_root": "0x0ff8...a12"
  },
  "ownership_zk_proof": {
    "protocol": "GROTH16_BLS12_381_DISCLOSURE_V1",
    "public_inputs": [
      "0x28af...110",
      "0x098bf...31a",
      "0x33bc...901",
      "0x0000...8f12c4",
      "0x0000...99a1"
    ],
    "proof_bytes": "0x91ab...772c"
  }
}
```

### B. Skema JSON Laporan Audit B2B (`zeltra_b2b_audit_report_v1.json`)

Laporan audit B2B memisahkan rincian biaya (`protocol_fee` vs `execution_fee`) serta melacak perubahan UTXO (`input_notes_amount` dan `change_note_amount`) agar persamaan akuntansi selalu seimbang (*balanced double-entry*):

$$\text{OpeningBalance} + \text{TotalInflow} - (\text{TotalMerchantPayout} + \text{TotalProtocolFees} + \text{TotalExecutionFees}) == \text{ClosingBalance}$$

```json
{
  "schema_version": "zeltra.compliance.audit.v1",
  "organization_name": "Acme Corp AI Agent Fleet",
  "report_period": {
    "from_timestamp": 1773532800,
    "to_timestamp": 1776211200
  },
  "summary_by_asset": {
    "usdc": {
      "asset_id": 1,
      "symbol": "USDC",
      "opening_balance": "0.000000",
      "total_inflow_deposit": "50000.000000",
      "total_merchant_payout": "10000.000000",
      "total_protocol_fees": "45.000000",
      "total_relayer_execution_fees": "0.500000",
      "closing_unspent_balance": "39954.500000"
    },
    "weth": {
      "asset_id": 0,
      "symbol": "WETH",
      "opening_balance": "0.000000000",
      "total_inflow_deposit": "5.000000000",
      "total_merchant_payout": "1.000000000",
      "total_protocol_fees": "0.004500000",
      "total_relayer_execution_fees": "0.000150000",
      "closing_unspent_balance": "3.995350000"
    }
  },
  "records": [
    {
      "record_id": 1,
      "timestamp": 1773619200,
      "tx_hash": "0x918f...112",
      "direction": "INFLOW_DEPOSIT",
      "asset_id": 1,
      "symbol": "USDC",
      "input_notes_total": "0.000000",
      "deposit_amount": "50000.000000",
      "merchant_payout": "0.000000",
      "protocol_fee": "0.000000",
      "relayer_execution_fee": "0.000000",
      "change_amount": "0.000000",
      "created_commitments": ["0x12a9c...77f"],
      "spent_nullifiers": [],
      "recipient": null,
      "status": "SETTLED_ON_CHAIN"
    },
    {
      "record_id": 2,
      "timestamp": 1774051200,
      "tx_hash": "0xca21...884",
      "direction": "OUTFLOW_SPEND",
      "asset_id": 1,
      "symbol": "USDC",
      "input_notes_total": "50000.000000",
      "deposit_amount": "0.000000",
      "merchant_payout": "10000.000000",
      "protocol_fee": "45.000000",
      "relayer_execution_fee": "0.500000",
      "change_amount": "39954.500000",
      "created_commitments": ["0x99aa...221"],
      "spent_nullifiers": ["0x33bc...901"],
      "recipient": "0xMerchantReceivingAddress...",
      "status": "SETTLED_ON_CHAIN"
    }
  ]
}
```

### C. Format Ekspor CSV B2B Sanitized (`zeltra_audit_report.csv`)
Untuk impor langsung ke SAP, Xero, QuickBooks, atau Microsoft Excel dengan sanitasi formula injection:
```csv
RecordID,Timestamp,DateISO,Direction,AssetID,AssetSymbol,GrossAmount,FeePaid,NetAmount,Recipient,TxHash,Commitment,Nullifier,Status
1,1773619200,2026-03-15T00:00:00Z,INFLOW_DEPOSIT,1,USDC,50000.000000,0.000000,50000.000000,N/A,0x918f...112,0x12a9c...77f,N/A,SETTLED
2,1774051200,2026-03-20T00:00:00Z,OUTFLOW_SPEND,1,USDC,10000.000000,45.500000,9954.500000,0xMerchant...,0xca21...884,0x99aa...221,0x33bc...901,SETTLED
```

---

## 6. Antarmuka Tooling CLI & SDK

### A. Perintah CLI (`zeltra-cli`)
```bash
# 1. Export Laporan Audit B2B (Format JSON atau CSV)
zeltra-cli export-audit \
  --format json \
  --asset all \
  --from 2026-01-01 \
  --to 2026-03-31 \
  --out ./audit_q1_2026.json

# 2. Export Bukti Sumber Dana untuk CEX (Tier 1: Single-Note Selective Disclosure)
zeltra-cli export-cex-proof \
  --commitment 0x12a9c...77f \
  --verifier "BINANCE_KYC_DESK" \
  --account-uid "USER-98124712" \
  --spent-tx 0x44ca...990 \
  --recipient 0xBinanceDepositAddress... \
  --out ./binance_proof.json

# 3. Verifikasi Bukti CEX (Dapat dijalankan staf CEX / auditor tanpa wallet Zeltra)
zeltra-cli verify-cex-proof \
  --file ./binance_proof.json \
  --expected-verifier "BINANCE_KYC_DESK"

# 4. Export Viewing Key Delegation (Tier 3: Read-Only Internal Auditor)
zeltra-cli export-viewing-key \
  --type fvk \
  --out ./auditor_delegation.key
```

### B. Antarmuka SDK (`zeltra-sdk`)
Modul `zeltra_sdk::compliance`:
- `export_audit_trail(&self, filter: AuditFilter) -> Result<AuditReport, WalletError>`
- `export_audit_csv(&self, filter: AuditFilter) -> Result<String, WalletError>`
- `create_cex_selective_proof(&self, commitment: [u8; 32], verifier: VerifierBinding) -> Result<CexProofPayload, WalletError>`
- `verify_cex_selective_proof(payload: &CexProofPayload, rpc_client: &EvmClient) -> Result<VerificationSummary, ComplianceError>`
- `export_full_viewing_key(&self) -> Result<FullViewingKey, WalletError>`
- `import_full_viewing_key(fvk: &FullViewingKey) -> Result<ReadOnlyAuditorSession, WalletError>`

---

## 7. Strict Security Invariants (Aturan Tak Boleh Dilanggar)

1. **INVARIANT 1 (Absolute Zero Secret Leakage):**
   Fungsi ekspor audit dan proof generator **dilarang keras** memuat field rahasia master (`spending_key`, `seed`, `wallet_password`, `argon2_hash`, atau SQLite cipher key) pada output JSON, CSV, atau log console.
2. **INVARIANT 2 (Verifier Binding Anti-Replay):**
   Setiap berkas pembuktian Tier 1 wajib mengikat `verifier_id`, `account_uid`, `spent_tx_hash`, `payout_recipient`, `nullifier`, dan rentang kedaluwarsa waktu. Pembuktian tanpa verifier binding lengkap harus ditolak oleh mesin verifikasi.
3. **INVARIANT 3 (Strict Asset Partitioning):**
   Perhitungan saldo dan arus kas wajib terpisah secara kaku antara `asset_id = 0` (WETH) dan `asset_id = 1` (USDC). Dilarang menjumlahkan atau mencampuradukkan angka dari kedua aset tersebut dalam satu kolom numerik.
4. **INVARIANT 4 (Cryptographic Preimage Integrity):**
   Nilai komitmen yang diekspor wajib dihitung menggunakan kanonikal bit-packing `value_packed = v + asset_id * 2^64` dan diverifikasi ulang dengan hasher Poseidon-W5 sebelum laporan disahkan.
5. **INVARIANT 5 (Anti-CSV Formula Injection):**
   Saat mengekspor laporan ke format CSV (`export_audit_csv`), setiap nilai string yang diawali dengan karakter pemicu formula spreadsheet (`=`, `+`, `-`, `@`, `\t`, `\r`) **wajib** disanitasi (di-strip atau diberi prefix tanda kutip tunggal `'`) untuk mencegah serangan *CSV Formula Injection* pada komputer auditor.
6. **INVARIANT 6 (Double-Entry UTXO Conservation Check):**
   Sebelum menghasilkan berkas laporan B2B, fungsi `export_audit_trail` wajib memverifikasi persamaan konservasi UTXO pada setiap record `OUTFLOW_SPEND`:
   $$\text{input\_notes\_total} == \text{merchant\_payout} + \text{protocol\_fee} + \text{relayer\_execution\_fee} + \text{change\_amount}$$
   Jika terdapat selisih 1 unit terkecil sekalipun (1 Gwei WETH atau 1 Micro USDC), proses ekspor wajib gagal secara *fail-closed* (`ComplianceError::AccountingReconciliationMismatch`).

---

## 8. Referensi & Landasan Akademik

1. **Zcash Protocol Specification (Version 2024.5.1 [NU5/Orchard]):**
   - Section 4.2: *Key Components & Viewing Key Derivation*.
   - Section 4.19: *In-band Secret Distribution and Note Plaintexts*.
   - Ref: D. Hopwood, S. Bowe, E. Taylor, N. Wilcox. Electric Coin Company, 2024. URL: [https://zips.z.cash/protocol/protocol.pdf](https://zips.z.cash/protocol/protocol.pdf)
2. **Zcash Improvement Proposal 310 (ZIP 310):**
   - *Security Properties of Sapling Viewing Keys*.
   - Ref: S. Bowe, D. Hopwood. Electric Coin Company, 2024. URL: [https://zips.z.cash/zip-0310](https://zips.z.cash/zip-0310)
3. **Zcash Improvement Proposal 311 (ZIP 311):**
   - *Zcash Payment Disclosures: Non-Interactive Proofs of Payment for Shielded Transactions*.
   - Ref: J. Grigg, S. Bowe. Electric Coin Company, 2024. URL: [https://zips.z.cash/zip-0311](https://zips.z.cash/zip-0311)
4. **Zcash Improvement Proposal 304 (ZIP 304):**
   - *Signature Proofs for Sapling Payment Addresses*.
   - Ref: J. Grigg. Electric Coin Company, 2020. URL: [https://zips.z.cash/zip-0304](https://zips.z.cash/zip-0304)
5. **Tornado Cash Vulnerability Post-Mortem (Februari 2020):**
   - *Vulnerability Disclosure: Private Note Leakage via Referer Headers and Web Compliance Misconfiguration*.
   - Ref: Tornado Cash Core Team, 2020. URL: [https://tornado-cash.medium.com/vulnerability-disclosure-f610fb7f2c8d](https://tornado-cash.medium.com/vulnerability-disclosure-f610fb7f2c8d)
6. **Tornado Cash Compliance Tool Documentation:**
   - *Tornado.Cash Compliance Tool: Proving Origin of Funds Without Revealing Entire Wallet History*.
   - Ref: Tornado Cash Documentation, 2021. URL: [https://docs.tornado.cash/tornado-cash-classic/compliance-tool](https://docs.tornado.cash/tornado-cash-classic/compliance-tool)
7. **RFC 8439 / RFC 5869:**
   - *ChaCha20 and Poly1305 for IETF Protocols* (AEAD Authenticated Encryption). URL: [https://datatracker.ietf.org/doc/html/rfc8439](https://datatracker.ietf.org/doc/html/rfc8439)
   - *HMAC-based Extract-and-Expand Key Derivation Function (HKDF)*. URL: [https://datatracker.ietf.org/doc/html/rfc5869](https://datatracker.ietf.org/doc/html/rfc5869)
8. **Zeltra Architecture Decisions:**
   - [`DEC-026: Receiver-Enforced Modular ASP & Relayer Compliance`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)
   - [`DEC-032: Merkle Mountain Range Commitment Accumulator`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-032-merkle-mountain-range-commitment-accumulator.md)
   - [`DEC-034: Dual-Asset Architecture (USDC & WETH Rails)`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md)
   - [`DEC-034A: Canonical Field Bit-Packing and Intra-Asset Circuit Constraints`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md)
   - [`DEC-035A: Two-Limb Quote Hash Representation and Range Constraints`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md)
   - [`DEC-037: Note Encryption Memo Protocol & Viewing Key Recovery`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-037-note-encryption-memo-and-viewing-key-recovery.md)
   - [`DEC-038: Pure ZK-UTXO Migration & Direct MMR Deposit`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
   - [`DEC-039: Memo Integrity Gap Mitigation`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-039-MEMO_INTEGRITY_GAP.md)

---

## 9. Matriks Pengujian Formal (`zeltra-sdk` & `zeltra-core`)

Implementasi DEC-040 wajib menyertakan pengujian berikut (rasio negatif $\ge 2\times$ positif):

### Positive Tests:
1. `test_cex_selective_proof_roundtrip_usdc_and_weth`: Ekspor bukti Tier 1 untuk note USDC (`asset_id = 1`) dan WETH (`asset_id = 0`) lulus seluruh 5 tahap verifikasi deterministik.
2. `test_b2b_audit_report_reconciliation_balance`: Ekspor laporan multi-transaksi menghasilkan `closing_unspent_balance` yang sama persis dengan saldo `get_balance(asset_id)` di dompet.
3. `test_csv_export_standard_accounting_format`: Ekspor CSV berhasil di-parse kembali tanpa kehilangan presisi desimal (9 desimal WETH, 6 desimal USDC).
4. `test_fvk_read_only_auditor_session`: Pendelegasian FVK (Tier 3) memungkinkan sesi auditor memindai dan merekonstruksi saldo masuk dan keluar tanpa dapat membelanjakan dana.

### Negative Tests (Security & Anti-Fraud):
1. `test_cex_proof_rejects_mismatched_verifier_id`: Bukti yang dibuat untuk `"BINANCE_KYC_DESK"` ditolak jika diverifikasi dengan `"COINBASE_COMPLIANCE"` (`VerifierIdMismatch`).
2. `test_cex_proof_rejects_expired_attestation`: Bukti dengan `timestamp > expires_at` ditolak seketika (`ProofExpired`).
3. `test_cex_proof_rejects_spoofed_nullifier_from_other_note`: Upaya menyuntikkan `nullifier` milik note lain ke dalam JSON gagal pada verifikasi Groth16 `ownership_zk_proof` (`InvalidDisclosureProof`).
4. `test_cex_proof_rejects_tampered_value_or_asset_id`: Mengubah `asset_id` dari `0` (WETH) menjadi `1` (USDC) di JSON gagal pada re-komputasi `value_packed` dan komitmen Poseidon-W5 (`CommitmentPreimageMismatch`).
5. `test_cex_proof_rejects_forged_mmr_sibling_path`: Bukti dengan komitmen fiktif yang tidak ada di pohon MMR gagal pada pencocokan `mmr_root` (`InvalidMmrInclusionProof`).
6. `test_export_never_contains_spending_key_bytes`: Inspeksi byte-level pada output JSON dan CSV memastikan byte `spending_key` tidak muncul di bagian mana pun dari berkas ekspor.
7. `test_csv_export_sanitizes_formula_injection_payloads`: Nama organisasi atau label yang diawali `=cmd|...` otomatis disanitasi sebelum ditulis ke CSV.
8. `test_fvk_cannot_generate_spend_proof`: Sesi auditor hanya dengan FVK ditolak secara instan saat mencoba memanggil fungsi pembuatan proof spend (`MissingSpendAuthority`).
