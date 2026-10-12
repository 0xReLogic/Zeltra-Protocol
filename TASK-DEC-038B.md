# TASK-DEC-038B: Stylus Smart Contract Direct MMR Inflow & Storage Purification Execution Checklist

> **Spesifikasi Arsitektur Referensi:**
> - Master Blueprint: [`DEC-038`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
> - Sub-Spesifikasi Formal: [`DEC-038B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md)

---

## Mandat Utama
**Atomic Inflow & Absolute Solvency:**
- Menggantikan alur 2-Step Deposit/Reveal lama dengan entrypoint tunggal on-chain: `deposit_direct_note`.
- Komitmen note langsung disisipkan ke dalam Merkle Mountain Range (`_mmr_insert`) dan saldo note langsung berstatus UNSPENT dalam 1 blok Arbitrum (~250 ms).
- Solvensi disederhanakan murni: $\text{USDC Balance} \ge \text{user\_note\_liability} + \text{accrued\_execution\_fee\_liability} + \text{realized\_protocol\_fees}$.

---

## Checklist Eksekusi Step-by-Step

### Step 1: Implementasi Entrypoint `deposit_direct_note`
- [x] Buat fungsi `deposit_direct_note(amount: U256, note_commitment: FixedBytes<32>, encrypted_memo: Bytes)` di `nimbus-contracts/src/deposit.rs`.
- [x] Pasang guard darurat: `self.check_not_paused()?;`.
- [x] Validasi nominal deposit minimum (10 USDC = `10_000_000` micro-units):
  ```rust
  if amount < U256::from(10_000_000) {
      return Err(b"AMOUNT_TOO_SMALL".to_vec());
  }
  ```
- [x] Validasi anti-Nomad zero-root exploit:
  ```rust
  if note_commitment == FixedBytes::<32>::ZERO {
      return Err(b"ZERO_COMMITMENT_REJECTED".to_vec());
  }
  ```
- [x] Validasi batas ukuran encrypted memo ($64 \le \text{len} \le 256$ byte):
  ```rust
  if encrypted_memo.len() < 64 || encrypted_memo.len() > 256 {
      return Err(b"INVALID_MEMO_LENGTH".to_vec());
  }
  ```

---

### Step 2: Sanitasi Skalar Kanonikal (Anti-Aliasing Modulus EIP-2537)
- [x] Validasi elemen medan $\mathbb{F}_r$ menggunakan helper canonical deserialization:
  ```rust
  if crate::types::from_evm_scalar(&note_commitment.0).is_none() {
      return Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec());
  }
  ```
- [x] Pastikan penolakan bersifat *fail-closed* jika byte bernilai $x \ge r$.

---

### Step 3: Transfer Aset ERC-20 & Akuntansi Liabilitas
- [x] Eksekusi transfer token USDC dari `msg_sender()` ke kontrak via `erc20.transfer_from`:
  ```rust
  let success = erc20.transfer_from(&host, Call::new_mutating(self), client, this_address, amount)?;
  if !success {
      return Err(b"TRANSFER_FROM_FAILED".to_vec());
  }
  ```
- [x] Mutasi akumulator liabilitas:
  - `total_deposited_principal += amount`
  - `user_note_liability += amount`

---

### Step 4: Penyisipan Daun MMR On-Chain & Emisi Event
- [x] Sambungkan pemanggilan langsung ke `self._mmr_insert(note_commitment, encrypted_memo.to_vec())?;` di `nimbus-contracts/src/merkle.rs`.
- [x] Perbarui puncak `mmr_peaks`, perbarui `note_tree_root`, dan daftarkan root ke cincin riwayat `accepted_note_roots` dan `root_history`.
- [x] Pancarkan event log resmi:
  ```solidity
  event NoteCommitmentAppended(
      uint256 indexed leaf_index,
      bytes32 indexed commitment,
      bytes32 new_root,
      bytes encrypted_memo
  );
  ```

---

### Step 5: Penegasan Invarian Solvensi Mutlak
- [x] Sederhanakan fungsi `check_liability_invariant()`:
  ```rust
  let balance = erc20.balance_of(&host, this_address)?;
  let total_liabilities = self.user_note_liability.get()
      .checked_add(self.accrued_execution_fee_liability.get()).ok_or(b"OVERFLOW".to_vec())?
      .checked_add(self.realized_protocol_fees.get()).ok_or(b"OVERFLOW".to_vec())?;

  if balance < total_liabilities {
      return Err(b"INSOLVENT".to_vec());
  }
  ```
- [x] Panggil `check_liability_invariant()` di akhir fungsi `deposit_direct_note`.

---

### Step 6: Test Suite Kontrak ($2\times$ Negative Safety)
- [x] `POS-1`: Penyetoran valid $\rightarrow$ liabilitas bertambah, daun MMR bertambah, event terbit, solvensi lolos.
- [x] `NEG-1`: Penyetoran skalar non-kanonikal ($x \ge r$) $\rightarrow$ Revert `INVALID_NOTE_COMMITMENT_SCALAR` (state tidak berubah).
- [x] `NEG-2`: Penyetoran komitmen nol $\rightarrow$ Revert `ZERO_COMMITMENT_REJECTED` (state tidak berubah).
- [x] `NEG-3`: Nominal $< 10\text{ USDC}$ $\rightarrow$ Revert `AMOUNT_TOO_SMALL` (state tidak berubah).
- [x] `NEG-4`: Panjang memo $< 64$ atau $> 256$ byte $\rightarrow$ Revert `INVALID_MEMO_LENGTH` (state tidak berubah).
- [x] `NEG-5`: Deposit saat kontrak di-pause $\rightarrow$ Revert `CONTRACT_PAUSED` (state tidak berubah).
- [x] `NEG-6`: `transfer_from` return boolean `false` $\rightarrow$ Revert `TRANSFER_FROM_FAILED` (state tidak berubah).

---

## Verifikasi & Perintah Uji
```bash
cargo check -p nimbus-contracts
cargo test -p nimbus-contracts --lib -- --nocapture
cargo stylus check -p nimbus-contracts
```
