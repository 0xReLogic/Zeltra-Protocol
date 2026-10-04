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

---

## 4. Private Note Wallet & UTXO State (Gate E — DEC-024)

Untuk mendukung saldo fleksibel pecahan sembarang (*variable-amount ZK-UTXO*), SDK menyediakan `PrivateNoteWallet` yang mengelola siklus hidup note secara crash-safe dan melindungi privasi pengguna dari serangan *subset-sum / wallet fingerprinting*:

### A. Siklus Hidup Note 4 Tahap
$$\text{Unconfirmed} \longrightarrow \text{Unspent} \overset{\text{Reserve}}{\underset{\text{Rollback}}{\rightleftharpoons}} \text{Reserved} \longrightarrow \text{Spent}$$

* **`Unconfirmed`:** Note deposit baru atau kembalian (*change note*) yang belum terbit/terkonfirmasi on-chain.
* **`Unspent`:** Note confirmed on-chain dengan Merkle witness lengkap (siap dibelanjakan).
* **`Reserved`:** Note yang dikunci sementara untuk sesi belanja aktif dengan batas waktu sewa (`lease_expiry_secs = 120s`). Mencegah double-reservation antar tab browser.
* **`Spent`:** Note yang telah dikonsumsi di smart contract dengan nullifier terdaftar.

### B. Contoh Penggunaan Wallet Lengkap (TypeScript / Rust SDK):

```typescript
import { PrivateNoteWallet, NoteStatus } from 'nimbus-sdk';

// 1. Inisialisasi Wallet dari 32-Byte Master Seed (BIP-32 / DEC-024)
const seed = new Uint8Array(32); // Entropy aman (CSPRNG)
crypto.getRandomValues(seed);
const wallet = PrivateNoteWallet.new(seed);

// 2. Cek Saldo Gabungan (Aggregate Balance)
console.log("Saldo Tersedia:", wallet.balance());           // Hanya Unspent notes
console.log("Saldo Terkunci:", wallet.reserved_balance(now)); // Sesi aktif
console.log("Total Saldo:", wallet.total_balance(now));

// 3. Deposit Baru
const [depositNote, commitmentHex] = wallet.create_deposit_note(100_000_000, now);
// Kirim commitmentHex ke contract.deposit(100_000_000, commitmentHex)
// Setelah confirmed on-chain, update leaf_index dan Merkle path:
wallet.confirm_deposit(commitmentHex, leafIndex, merklePathArray, currentTreeRoot);

// 4. Seleksi Koin & Pembayaran (Coin Selection & Zero-Change Mode)
// Otomatis memilih note terbaik (Exact Match > Best Fit) untuk meminimalkan pecahan kembalian:
const selected = wallet.select_note_for_spend(
    25_000_000, // merchant payout
    112_500,    // protocol fee (45 bps)
    50_000,     // execution fee
    now
);

// 5. Pembuatan Proof ZK Lokal (Client-Side Groth16)
// Spending key & preimage tidak pernah dikirim ke relayer
const spendPayload = wallet.prepare_spend_proof(
    selected,
    sessionId,
    recipientAddress,
    maxExecutionFee,
    quoteHashHex,
    chainId,
    contractAddressHex,
    expiryTimestamp,
    now,
    provingKey
);

// 6. Two-Phase Commit (2PC) Crash Safety:
try {
    const txReceipt = await submitToRelayer(spendPayload);
    // Sukses: Promosikan change note menjadi Unspent, tandai input note Spent
    wallet.commit_spend(sessionId, txReceipt.nullifier, now, txReceipt.changeLeafIndex, txReceipt.changePath, txReceipt.newRoot);
} catch (err) {
    // Gagal / Timeout: Kembalikan input note ke status Unspent, batalkan change note
    wallet.rollback_spend(sessionId);
}

// 7. Backup & Recovery Terenkripsi (Password-Protected AES/HMAC)
const backupJson = wallet.export_backup("PasswordRahasia123!");
const restoredWallet = PrivateNoteWallet.import_backup(backupJson, "PasswordRahasia123!");
```
