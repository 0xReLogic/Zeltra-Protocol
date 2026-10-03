# Nimbus Contract — 02: Storage Layout & Tata Kelola

Dokumen ini menjelaskan tata letak storage on-chain pada [`nimbus-contracts/src/storage.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/storage.rs) dan mekanisme tata kelola (governance) bertimelock pada [`nimbus-contracts/src/lib.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/lib.rs).

---

## 1. Layout State Storage On-Chain (`storage.rs`)

Smart contract Stylus menggunakan makro `sol_storage!` untuk memetakan slot penyimpanan yang kompatibel dengan EVM:

```rust
sol_storage! {
    #[entrypoint]
    pub struct Nimbus {
        // --- Escrow & Deposit Sessions ---
        mapping(bytes32 => uint256) collateral;
        mapping(bytes32 => address) session_client;
        mapping(bytes32 => uint256) session_amount;
        mapping(bytes32 => bool)    session_resolved;
        mapping(bytes32 => uint256) session_timestamp;
        mapping(bytes32 => bytes32) session_commitment_hash; // Binding com_k (DEC-001)
        mapping(bytes32 => bool)    session_exists;

        // --- Nullifier & Anti-Double-Spend ---
        mapping(bytes32 => bool)    nullifiers;

        // --- ZK Compliance & Holding Time Discount ---
        mapping(bytes32 => uint256) clean_association_roots;

        // --- Financial & Principal Tracking ---
        uint256                     total_deposited_principal;
        address                     stablecoin;
        address                     fee_recipient;

        // --- Relayer Execution Fees ---
        uint256                     accumulated_execution_fees;
        address                     execution_fee_recipient;

        // --- Access Control & Timelocked Governance ---
        address                     owner;
        bool                        paused;
        address                     pending_owner;
        address                     proposed_fee_recipient;
        uint256                     fee_recipient_eta;

        // --- Security Registries (DEC-002 & DEC-015) ---
        mapping(bytes32 => bool)    trusted_issuer_keys;
        address                     ccip_router;
        mapping(bytes32 => bool)    ccip_processed_messages;
        mapping(bytes32 => bool)    ccip_allowed_senders;
        mapping(bytes32 => uint256) failed_intent_refunds;
    }
}
```

---

## 2. Tata Kelola Dua Langkah (*Two-Step Governance*)

Untuk mencegah kehilangan akses akibat salah ketik alamat admin (*fat-finger error*), pemindahan kepemilikan kontrak menerapkan prosedur **Two-Step Transfer**:

1. **`propose_owner(new_owner)`:** Owner memanggil fungsi ini untuk menetapkan calon pemilik baru pada slot `pending_owner`.
2. **`claim_ownership()`:** Hanya alamat yang tercatat di `pending_owner` yang dapat memanggil fungsi ini untuk menyelesaikan serah terima kepemilikan.

---

## 3. Parameter Perubahan Bertimelock (*24h Timelock*)

Pergantian alamat penampung fee protokol (`fee_recipient`) dilindungi oleh timelock minimal **24 jam (86.400 detik)** untuk mencegah eksploitasi instan jika kunci admin terkompromi:

1. **`propose_fee_recipient(recipient)`:** Owner mengajukan alamat baru. Kontrak mencatat:
   $$\text{fee\_recipient\_eta} = \text{block.timestamp} + 86.400$$
2. **`execute_fee_recipient_change()`:** Eksekusi hanya dapat dilakukan setelah waktu `block.timestamp >= fee_recipient_eta`.

---

## 4. Kontrol Darurat (*Emergency Pause*)

* **`pause()`:** Menghentikan sementara operasi `deposit`, `spend`, dan `batch_spend` jika terdeteksi anomali keamanan.
* **`unpause()`:** Mengaktifkan kembali operasi normal setelah mitigasi selesai.
* **Pengecualian Refund:** Alur refund escrow yang sudah melewati masa kadaluarsa 24 jam dirancang untuk tetap dapat diakses demi menjaga kepercayaan pengguna atas kepemilikan dana.

---

## 5. Registry Kunci Issuer Terpercaya (DEC-002)

Kontrak tidak menerima sembarang signature BLS dari pihak luar:
* **`register_trusted_issuer_key(pk_iss_bytes)`:** Admin mendaftarkan kunci publik $pk_{iss}$ (256 bytes) milik federasi Guardian yang sah.
* **`revoke_trusted_issuer_key(pk_iss_bytes)`:** Admin dapat mencabut kunci jika terjadi rotasi kunci atau kompromi guardian.
* Saat fungsi `spend()` dipanggil, kontrak memverifikasi:
  $$\text{trusted\_issuer\_keys}[\text{keccak256}(pk_{iss})] == \text{true}$$
  Transaksi akan seketika revert jika kunci issuer tidak terdaftar (`UNTRUSTED_ISSUER_KEY`).
