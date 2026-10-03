# Nimbus Contract — 03: Deposit, Reveal & Refund Lifecycle

Dokumen ini menjelaskan alur deposit kolateral, pembuktian kunci masking (*reveal*), dan penarikan refund darurat bertimelock pada [`nimbus-contracts/src/deposit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/deposit.rs).

---

## 1. Alur Deposit (`_deposit` & `_deposit_with_commitment`)

Fungsi `deposit()` / `deposit_with_commitment()` dipanggil oleh client untuk mengunci collateral USDC dan memulai sesi penerbitan kredensial privat (Gate D):

```text
Client memanggil deposit_with_commitment(sid, com_k_bytes, amount, note_commitment)
  ├── 1. Cek: Kontrak tidak dalam kondisi pause
  ├── 2. Cek: session_id belum pernah digunakan sebelumnya (Anti-Replay)
  ├── 3. Cek: Panjang komitmen com_k tepat 256 bytes (Titik G2)
  ├── 4. Cek: Nominal minimal 10 USDC (10_000_000 unit)
  ├── 5. Validasi: note_commitment adalah skalar kanonikal Fr
  ├── 6. Hitung fee deposit: 0 bps (0.00% - Zero-Friction Inflow)
  │           net_amount = amount (100% dana utuh)
  ├── 7. Transfer USDC dari client ke kontrak (Pull payment ERC-20)
  ├── 8. Catat state:
  │      • session_client = msg.sender
  │      • session_amount = net_amount
  │      • session_timestamp = block.timestamp
  │      • session_commitment_hash = keccak256(com_k) (Binding DEC-001)
  │      • session_note_commitment = note_commitment (Binding Gate D)
  │      • total_deposited_principal += net_amount
  │      • refundable_deposit_liability += net_amount
  ├── 9. Emit event DepositFee(sid, client, 0)
  └── 10. Verifikasi invariant solvabilitas (Assets >= Liabilities)
```

---

## 2. Alur Pembuktian Kunci Masking & Atomik Note Minting (`_reveal_mask_key`)

Setelah deposit USDC terkonfirmasi on-chain, leader merilis kunci masking $k$ ke blockchain melalui fungsi `reveal_mask_key()`:

### Mekanisme Verifikasi On-Chain:
Kontrak membuktikan bahwa $k$ yang diserahkan benar-benar menghasilkan komitmen $com_k$ yang telah dikunci saat deposit menggunakan precompile **BLS12_G2_MSM (`0x0c`)**:

$$\text{computed\_com\_k} = k \cdot pk_{iss} \in G_2$$

Kontrak memverifikasi kecocokan hash:
$$\text{keccak256}(\text{computed\_com\_k}) == \text{session\_commitment\_hash}[sid]$$

### Atomik Note Minting ke LeanIMT Merkle Tree (Gate D):
1. **Pergeseran Liabilitas:** Hak refund hangus; dana berpindah secara atomik:
   - `refundable_deposit_liability -= net_amount`
   - `user_note_liability += net_amount`
2. **Pencetakan Daun Merkle (Gated Minting):**
   Jika `session_note_commitment != 0`, kontrak memasukkan komitmen note ke pohon Merkle LeanIMT tingkat 20 secara on-chain (`insert_leaf`).
3. **Pembaruan Root Buffer:** Root baru dicatat ke riwayat *accepted note roots* bertimelock.
4. **Tanpa Transfer Kolateral:** Reveal **tidak memindahkan dana USDC keluar dari brankas**, menjamin collateral tetap 100% full-reserve.

---

## 3. Alur Klaim Refund Bertimelock (`_claim_refund`)

Jika federasi guardian offline, quorum 3/5 gagal tercapai, atau leader tidak pernah merilis kunci $k$, dana pengguna dijamin aman dan dapat ditarik kembali setelah melewati masa timelock:

```text
User memanggil claim_refund(sid)
  ├── 1. Cek: session_id terdaftar dan belum berstatus resolved
  ├── 2. Cek: msg.sender adalah client asli yang mendepositkan dana
  ├── 3. Cek: Timelock 24 jam (86.400 detik) telah lewat:
  │           block.timestamp >= session_timestamp + 86400
  ├── 4. Update status: session_resolved = true (Cegah klaim ganda)
  ├── 5. Kurangi liabilitas pokok:
  │           total_deposited_principal -= amount
  │           refundable_deposit_liability -= amount
  ├── 6. Transfer 100% net_amount USDC kembali ke dompet client (tanpa potongan)
  ├── 7. Emit event DepositRefunded(sid, client, amount)
  └── 8. Verifikasi invariant solvabilitas (Assets >= Liabilities)
```

### Jaminan Terminal State:
Dengan kombinasi flag `session_resolved` dan pengecekan timelock:
$$\text{SPENT} \oplus \text{REFUNDED} = \text{TRUE}$$
Sebuah sesi deposit hanya bisa berakhir di salah satu dari dua keadaan: berhasil dibelanjakan (*spent*) **ATAU** dikembalikan penuh (*refunded*). Tidak pernah bisa keduanya.
