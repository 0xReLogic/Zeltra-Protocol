# Nimbus Contract — 01: Overview & Arsitektur Stylus

Dokumen ini menjelaskan arsitektur, siklus hidup jaminan (*collateral*), dan aturan invariant otomatis dari smart contract Nimbus di Arbitrum Stylus ([`nimbus-contracts`](file:///workspaces/Zeltra-Protocol/nimbus-contracts)).

---

## 1. Lingkungan Arbitrum Stylus (Rust WASM)

Nimbus dibangun di atas **Arbitrum Stylus SDK 0.10.7**:
* **Eksekusi Native WASM:** Smart contract ditulis dalam bahasa Rust dan dikompilasi ke target `wasm32-unknown-unknown`, memberikan efisiensi komputasi hingga 10-100x lebih cepat dibanding Solidity EVM bytecode konvensional.
* **Precompiles Kriptografi (EIP-2537):** Stylus memanggil precompile native Ethereum untuk operasi pairing kurva BLS12-381 (`0x0f`) dan MSM $G_2$ (`0x0c`), menghemat biaya gas secara drastis saat memverifikasi tanda tangan.
* **Alloy Primitives & Types:** Menggunakan library `alloy-primitives` dan `alloy-sol-types` untuk kompatibilitas ABI standar ERC-20 dan pemanggilan lintas kontrak.

---

## 2. Struktur Modul Smart Contract (`nimbus-contracts/src/`)

```text
nimbus-contracts/src/
├── lib.rs            # Entrypoint utama Stylus, Public ABI, inisialisasi, & admin dispatch
├── storage.rs        # Layout storage state on-chain (Solidity-compatible layout)
├── deposit.rs        # Alur deposit USDC, reveal masking key, & refund timelock
├── spend.rs          # Verifikasi BLS pairing, rekonstruksi pesan, batching, & CCIP
├── verification.rs   # Verifikasi ZK-SNARK Groth16 on-chain via precompiles
├── vault.rs          # 100% Liquid Reserve (Full Reserve) buffer likuiditas
├── helpers.rs        # Private helpers (msg_sender, timestamp, hash_to_g1, check_owner)
├── interfaces.rs     # Definisi interface ERC-20 & ConditionalTokens (Alloy sol!)
├── constants.rs      # Alamat precompile EIP-2537 (0x0b..0x0f)
└── types.rs          # Formatter koordinat kurva EVM Big-Endian (128 & 256 bytes)
```

---

## 3. Siklus Hidup Jaminan (*Collateral Lifecycle* — DEC-001)

Prinsip dasar keuangan Nimbus: **Satu deposit hanya boleh keluar tepat satu kali (SPENT XOR REFUNDED), tidak lebih dan tidak kurang.**

```text
User Deposit USDC
  |
  +---> Kontrak mencatat liabilitas & commitment com_k
        |
        +---> Quorum Leader/Guardian menerbitkan masked signature
              |
              +---> Deposit confirmed on-chain
                    |
                    +---> Leader merilis masking key k
                          |
                          v
                    Alur Berhasil (Spend):
                    • User membuka kredensial (unmasking)
                    • User belanja ke merchant
                    • Kontrak verifikasi BLS pairing & nullifier
                    • Kontrak bayar merchant & potong liabilitas
                    [STATUS: SPENT - TERMINAL]
                          |
                    (Atau jika Quorum Gagal / k Tidak Dirilis):
                    • User menunggu timelock 24 jam (86.400 detik)
                    • User mengeksekusi claim_refund()
                    • Kontrak mengembalikan USDC & potong liabilitas
                    [STATUS: REFUNDED - TERMINAL]
```

### Aturan Emas Reveal (`revealMaskKey`):
Fungsi `reveal_mask_key` **TIDAK PERNAH mentransfer uang/kolateral**. Reveal hanya menyelesaikan status issuance, membuktikan $k \cdot pk_{iss} == com_k$, dan menutup hak refund. Ini mencegah eksploitasi ganda di mana uang ditarik saat reveal lalu ditarik lagi saat spend.

---

## 4. Invariant Solvabilitas Otomatis (DEC-009)

File referensi: [`nimbus-contracts/src/lib.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/lib.rs#L180-L205)

Setiap transaksi yang mengubah state (`deposit`, `refund`, `spend`, `claim_fee`) **wajib mengeksekusi pemeriksaan solvabilitas on-chain**:

$$\text{Saldo USDC Kontrak} \ge \text{Total Liabilitas Pokok User}$$

Jika saldo token USDC aktual di dalam kontrak kurang dari liabilitas yang dicatat, fungsi `check_liability_invariant` akan seketika melakukan **REVERT** dengan pesan error:
```text
INSOLVENT_LIABILITY_EXCEEDS_ASSETS
```
Ini memastikan kontrak tidak pernah mengalami insolvabilitas atau *bank run*.
