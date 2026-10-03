# Nimbus SDK — 02: Integrasi JavaScript / TypeScript

Panduan integrasi pustaka WebAssembly Nimbus SDK ke dalam aplikasi frontend berbasis browser (React, Next.js, Vue, atau Vite) dan backend Node.js.

---

## 1. Kompilasi Library ke WASM

Sebelum diimpor ke frontend, build package WASM menggunakan `wasm-pack`:

```bash
# Untuk aplikasi web berbasis bundler (Webpack / Vite):
wasm-pack build --target web --out-dir pkg

# Untuk aplikasi Node.js:
wasm-pack build --target nodejs --out-dir pkg-node
```

---

## 2. Inisialisasi & Siklus Kriptografi di Frontend

```typescript
import init, {
    wasm_client_blind,
    wasm_client_verify_masked,
    wasm_client_unmask,
    wasm_client_verify_final,
    wasm_aggregate_shares
} from 'nimbus-sdk';

async function main() {
    // 1. Inisialisasi runtime WASM di browser
    await init();
    console.log("Nimbus WASM Runtime Siap!");

    // 2. Client Blinding Pesan Pembayaran
    const message = new TextEncoder().encode("ephemeral_payment_intent_456");
    const blindedResult = wasm_client_blind(message);
    
    const blindedHex = blindedResult.blinded_hex; // Titik kurva G1 (X)
    const rHex = blindedResult.r_hex;             // Faktor pembuta privat (r)

    // 3. Kirim X ke Relayer / Leader Node
    const signResponse = await fetch("https://relayer.nimbus.network/api/leader/sign", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ blinded_hex: blindedHex })
    });
    const { masked_sig_hex, com_k_hex } = await signResponse.json();

    // 4. Verifikasi Masked Signature Off-Chain (Keamanan Client)
    const isValid = wasm_client_verify_masked(blindedHex, com_k_hex, masked_sig_hex);
    if (!isValid) {
        throw new Error("Peringatan: Tanda tangan dari cluster relayer tidak valid!");
    }
    console.log("Masked signature terverifikasi valid!");

    // 5. Eksekusi Deposit On-Chain USDC ke Smart Contract
    // (Panggil fungsi contract: deposit(sid, com_k, amount))
    // ...

    // 6. Dapatkan Masking Key k dari Leader setelah Deposit Confirmed
    const revealResponse = await fetch("https://relayer.nimbus.network/api/reveal", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ session_id: "..." })
    });
    const { k_hex } = await revealResponse.json();

    // 7. Unmasking untuk Mendapatkan Kredensial Final Bersih (alpha)
    const alphaHex = wasm_client_unmask(masked_sig_hex, rHex, kHex);
    console.log("Kredensial Final (BLS Signature):", alphaHex);

    // Kredensial ini sekarang siap dikirim untuk membayar merchant!
}
```

---

## 3. Agregasi Threshold Signature di Sisi Klien

Jika client berinteraksi langsung dengan federasi beberapa guardian secara independen, client dapat menggabungkan partial signatures di memori lokal:

```typescript
// Menggabungkan 3 partial signature dari guardian 1, 2, dan 4:
const indices = "1,2,4";
const signatures = `${sig1Hex},${sig2Hex},${sig4Hex}`;

const aggregatedMaskedSig = wasm_aggregate_shares(indices, signatures);
```
Fungsi ini menghitung interpolasi Lagrange secara instan di dalam WASM tanpa membocorkan secret share antar guardian.
