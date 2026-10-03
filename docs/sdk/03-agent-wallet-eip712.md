# Nimbus SDK — 03: EIP-712 Quotes & AI Agent Wallet (x402)

Dokumen ini menjelaskan implementasi penandatanganan kuotasi berbasis **EIP-712** pada [`nimbus-sdk/src/eip712.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/eip712.rs) dan integrasi pembayaran mikro otonom untuk agen AI pada [`nimbus-sdk/src/x402/`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/x402/).

---

## 1. Penandatanganan Kuotasi Berbasis EIP-712 (`eip712.rs`)

Untuk melindungi pengguna dari biaya gas tersembunyi (*hidden markup*), relayer menerbitkan kuotasi biaya eksekusi bertipe (*typed data*) menggunakan standar **EIP-712**.

### Skema Data `ExecutionQuoteEip712`:
```solidity
struct ExecutionQuoteEip712 {
    uint256 quote_id;           // Identifier unik kuotasi
    address relayer;            // Alamat relayer yang akan memproses transaksi
    address recipient;          // Alamat penerima pembayaran (merchant)
    uint256 amount;             // Nominal invoice yang dibayarkan
    uint256 protocol_fee;       // Biaya protokol (0.25% atau diskon holding)
    uint256 max_execution_fee;  // Batas toleransi maksimal gas reimbursement
    uint256 quote_expiry;       // Timestamp kedaluwarsa kuotasi (TTL)
    uint256 chain_id;           // Chain ID (Arbitrum Sepolia / Arbitrum One)
    address contract_address;   // Alamat smart contract Stylus
    bytes32 association_root;   // Root waktu simpan untuk diskon fee
}
```

### Domain Separator:
* `name`: `"Nimbus Execution Quote"`
* `version`: `"1"`
* `chainId` & `verifyingContract`: Terikat kuat ke jaringan dan alamat kontrak target (*domain separation*).

Pengguna menandatangani struct ini menggunakan wallet Web3 (Metamask, Rabby, atau private key via `k256`), memastikan relayer tidak dapat mengubah parameter harga atau rute setelah persetujuan diberikan.

---

## 2. AI Agent Spending Wallet & Protokol x402

File referensi: [`nimbus-sdk/src/x402/`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/x402/)

Nimbus SDK menyediakan pustaka self-custodial bagi autonomous agent AI untuk melakukan pembayaran API secara otomatis saat menemui kode status **HTTP 402 Payment Required**:

```text
[ AI Agent HTTP Client ]                       [ API Provider / LLM Server ]
           |                                                 |
           | 1. Request API (misal query data / LLM call)   |
           |------------------------------------------------>|
           |                                                 |
           | 2. Response: HTTP 402 Payment Required          |
           |    Header: WWW-Authenticate: Nimbus-BAT ...     |
           |<------------------------------------------------|
           |                                                 |
           | 3. SDK mendeteksi invoice 402:                  |
           |    • Ambil private note dari wallet lokal       |
           |    • Buat ZK / BLS payment payload              |
           |    • Encode payload ke Base64 JSON              |
           |                                                 |
           | 4. Retry Request dengan Header Pembayaran:      |
           |    Header: Authorization: Nimbus-BAT <payload>  |
           |------------------------------------------------>|
           |                                                 |
           | 5. Provider memverifikasi / meredeem payment    |
           |    Response: HTTP 200 OK + Data API             |
           |<------------------------------------------------|
```

### Keunggulan untuk Developer AI:
1. **Self-Custodial:** Kunci rahasia note disimpan lokal di memori agent, bukan di server terpusat.
2. **Tanpa Interupsi Manusia:** Agent dapat membelanjakan saldo secara otomatis sesuai batasan limit kebijakan (*spending policy guard*) yang ditetapkan pemilik.
3. **Privasi Penuh:** API provider tidak dapat menghubungkan dompet utama penyandang dana (*funder*) dengan bot AI yang memanggil layanan.
