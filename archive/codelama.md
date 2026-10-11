# Catatan Teknis & Utang Teknis: Transisi Arsitektur LeanIMT ke MMR

Dokumen ini mencatat artefak kode lama (*vestigial / legacy code*) dari masa transisi **Gate D (LeanIMT Depth 20)** menuju **Gate C0/DEC-032/DEC-036 (Merkle Mountain Range / MMR)** agar tidak terlupakan saat refactoring atau sebelum MPC Trusted Setup ceremony.

---

## 1. Kasus & Konteks Masalah

Sebelum migrasi ke **DEC-032 (Merkle Mountain Range / MMR)** dan **DEC-036 (Universal 2-in-2-out JoinSplit)**, Zeltra (Nimbus) menggunakan pohon komitmen berukuran tetap (*fixed depth* 20) berbasis LeanIMT:
* Maksimal kapasitas daun: $2^{20} = 1.048.576$ daun.
* Panjang bukti keanggotaan Merkle (*Merkle membership proof*): tepat 20 sibling (`[Fr; 20]`).

Setelah di-upgrade ke MMR on-chain:
* Kontrak on-chain menggunakan `_mmr_insert` dengan peak bagging dinamis berkapasitas hingga $2^{64}$ daun.
* ZK Circuit (`note_circuit.rs` dan `joinsplit_circuit.rs`) membuktikan bukti internal mountain (`mountain_siblings: [Fr; 32]`) dan peak bagging (`peak_bagging_siblings: [Fr; 32]`).
* SDK membangkitkan MMR proof via `self.mmr.generate_proof(leaf_index)` atau `cached_mmr_proof`.

---

## 2. Lokasi Sisa Kode Lama (Vestigial / Legacy Code)

### A. Client SDK: `nimbus-sdk/src/wallet/note_wallet.rs`
1. **Validasi Panjang Path di `insert_or_update_note` & `update_note_witness`:**
   ```rust
   // Baris 570-576 & 611-617
   if merkle_path.len() != MERKLE_TREE_DEPTH {
       return Err(WalletError::CryptoError(format!(
           "Invalid Merkle path length: expected {}, got {}",
           MERKLE_TREE_DEPTH,
           merkle_path.len()
       )));
   }
   ```
2. **Dead Field Validation di `prepare_private_spend`:**
   ```rust
   // Baris 1217-1219
   if merkle_path_strings.len() != MERKLE_TREE_DEPTH {
       return Err(WalletError::CryptoError("Invalid Merkle path depth".into()));
   }
   ```
   **Kejanggalan/Masalah:**
   Variabel `merkle_path_strings` divalidasi harus tepat 20 elemen, **tetapi setelah baris tersebut variabel sama sekali tidak dipakai**. Eksekusi langsung melompat memakai MMR murni:
   ```rust
   let mmr_proof = self.mmr.generate_proof(leaf_index as usize);
   // mengambil mountain_siblings [Fr; 32] dan peak_bagging_siblings [Fr; 32]
   ```
3. **Field Struct `WalletNote`:**
   ```rust
   pub merkle_path_hex: Option<Vec<String>>,
   ```
   Field ini masih ada di struct dan dicek di coin selection (`if n.merkle_path_hex.is_none()`), padahal bukti pohon riil sekarang disimpan dalam MMR store/cache.

---

### B. Core & Kontrak
1. **`nimbus-core/src/note.rs` & `poseidon_w5_constants.rs`:**
   - Konstanta `pub const MERKLE_TREE_DEPTH: usize = 20;`
   - Array `EMPTY_SUBTREE_HASHES_BYTES: [[u8; 32]; 21]`
   - Fungsi `compute_merkle_root` lama yang masih diekspor untuk backward-compatibility.
2. **`nimbus-contracts/src/merkle.rs`:**
   - Fungsi `_merkle_insert` dipertahankan sebagai alias inline ke `_mmr_insert`.
   - `EMPTY_TREE_ROOT_BYTES` (hash root pohon kosong depth 20) masih dipakai sebagai fallback canonical root saat `mmr_leaf_count == 0`.

---

## 3. Rencana Refactoring (Action Items untuk Gate Freeze / Ceremony)

1. [ ] **Deprecate `merkle_path_hex` di `WalletNote`:**
   Gantikan dengan `Option<MmrProofData>` atau pastikan status note diverifikasi lewat keberadaan `leaf_index` dan cache MMR.
2. [ ] **Hapus Pengecekan Hardcoded `MERKLE_TREE_DEPTH == 20` di SDK:**
   Hapus `if merkle_path.len() != MERKLE_TREE_DEPTH` di `note_wallet.rs` agar wallet tidak membatasi ukuran path secara artifisial.
3. [ ] **Update Unit Test SDK:**
   Ubah mock test di `note_wallet.rs` dan `coin_selection.rs` yang masih mengisi `merkle_path_hex: Some(vec!["0x04".to_string(); 20])` agar menggunakan MMR witness builder.
4. [ ] **Pembersihan Konstanta:**
   Pindahkan konstanta depth 20 ke modul deprecated atau isolasi khusus fallback testing.
