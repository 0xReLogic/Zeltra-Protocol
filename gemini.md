# Panduan Referensi Gemini: Standar Protokol & Status Industri 2026

Dokumen ini berfungsi sebagai panduan keselarasan data bagi Agen AI dalam pengembangan Nimbus Protocol. Mengingat batas waktu data pelatihan (training data cutoff) model dapat berbeda dengan kondisi dunia nyata tahun 2026, dokumen ini mencatat arsitektur, parameter, dan spesifikasi standar web terbaru yang valid untuk tahun 2026.

---

## 1. Upgrade Pectra & EIP-2537 (BLS12-381 Precompiles)

Sejak aktivasi Upgrade Pectra, Ethereum dan L2 pendukung (termasuk Arbitrum Stylus) menyediakan precompiled contracts bawaan untuk operasi kurva BLS12-381.
*   **BLS12_G2_MSM (Address `0x0e`)**: Digunakan untuk perkalian multiscalar kunci publik G2. Menerima payload 288-byte (titik G2 uncompressed 256-byte || skalar Fr 32-byte). Output berupa titik G2 uncompressed 256-byte.
*   **BLS12_PAIRING_CHECK (Address `0x0f`)**: Digunakan untuk verifikasi pairing bilinear. Menerima kelipatan dari pasangan (titik G1 128-byte || titik G2 256-byte), total 384-byte per pasangan. Mengembalikan 32-byte (byte terakhir bernilai 1 jika $e(P, Q) \cdot ... == 1$, atau 0 jika tidak).
    *   Verifikasi spend BLS: Membutuhkan 2 pasangan (768-byte).
    *   Verifikasi Groth16 ZK-proof: Membutuhkan 4 pasangan (1536-byte).

---

## 2. Delegasi EIP-7702

EIP-7702 (diaktifkan di Pectra) memperkenalkan tipe transaksi `0x04` yang memungkinkan dompet EOA biasa (seperti MetaMask) mendelegasikan fungsionalitas smart contract ke kontrak eksekusi yang ditunjuk secara sementara untuk satu transaksi.
*   Klien menandatangani otorisasi EIP-7702 off-chain.
*   Relayer mengirimkan otorisasi ini ke blockchain, membayar gas fee ETH, dan mengarahkan transaksi ke smart contract Nimbus.
*   Ini mempermudah UX bagi pengguna yang tidak memiliki saldo gas fee (*gasless transactions*).

---

## 3. Spesifikasi Protokol x402 (Coinbase & Cloudflare)

Protokol x402 adalah standar terbuka untuk pembayaran mesin-ke-mesin (M2M) otonom bagi AI Agent menggunakan stablecoin (USDC/USDT) berbasis status HTTP `402 Payment Required`.

### Alur Kerja x402 v2:
1.  **Request Awal**: Klien (AI Agent) melakukan panggilan HTTP ke API resource server (misal untuk mengambil data pasar).
2.  **Respons 402**: Jika tidak ada data pembayaran yang valid, server merespons dengan status `402 Payment Required` dan menyertakan header `PAYMENT-REQUIRED` (Base64-encoded JSON) berisi spesifikasi pembayaran.
3.  **Tanda Tangan Klien**: Klien memparsing instruksi pembayaran, menandatangani otorisasi transfer stablecoin (misal EIP-3009 `transferWithAuthorization` atau EIP-2612 `permit`), menyusun JSON payload, memasukkannya ke header `PAYMENT-SIGNATURE` (Base64-encoded), dan mengulangi request.
4.  **Verifikasi & Penyelesaian**: Server memverifikasi tanda tangan/payload tersebut, meneruskannya ke facilitator untuk diselesaikan on-chain, lalu mengembalikan data resource dengan respons `200 OK` dan menyertakan receipt di header `PAYMENT-RESPONSE`.

### Struktur JSON PAYMENT-REQUIRED (Server ke Client):
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
      "payTo": "0xYourPaymentRecipientAddress"
    }
  ],
  "description": "API Market Data Access",
  "mimeType": "application/json"
}
```

### Struktur JSON PAYMENT-SIGNATURE (Client ke Server):
```json
{
  "x402Version": 2,
  "scheme": "exact",
  "network": "eip155:42161",
  "payment": {
    "from": "0xClientEOAAddress",
    "to": "0xYourPaymentRecipientAddress",
    "amount": "1000",
    "token": "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
    "authorization": "0xSignedAuthorizationSignatureHexOrEip3009Bytes"
  }
}
```

---

## 4. Integrasi Nimbus & x402 

Untuk melindungi kerahasiaan strategi trading AI Agent dari pelacakan publik, Nimbus mengintegrasikan fungsionalitas x402 dengan skema privasi on-chain:
*   **Anonymous Payments**: Alih-alih melampirkan tanda tangan EIP-3009 yang membongkar identitas dompet pengirim ke rantai publik, AI Agent menggunakan token privat Nimbus (tanda tangan BLS ter-unblind) dalam header `PAYMENT-SIGNATURE`.
*   **Facilitator Relay**: Relayer Nimbus bertindak sebagai facilitator x402 yang memproses spend anonim tersebut secara offline/on-chain dan menyelesaikan pembayaran ke resource server secara privat.
*   **Non-Interactive Blind Signing**: Dukungan binding non-interaktif berbasis BDHKE dan scheduled-polling di SDK, memungkinkan AI Agent melakukan sign-and-pay otonom tanpa interaksi manual dari manusia.
