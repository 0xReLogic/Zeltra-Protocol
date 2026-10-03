# Nimbus Core — 03: Private Note UTXO & Merkle Tree

Dokumen ini menjelaskan struktur data **Private Note V1**, skema komitmen kriptografi, derivasi nullifier, dan struktur Merkle tree yang diimplementasikan pada [`nimbus-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note.rs) (mengacu pada spesifikasi [`DEC-016`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) & `DEC-016A`).

---

## 1. Masalah yang Diselesaikan: Sisa Kembalian (*Change Output*)

Pada model voucher lama, satu kredensial hanya bisa dipakai untuk satu nominal pasti dan hangus sekali pakai.

Dengan model **Private Note UTXO (Jalur B)**:
* Dana pengguna direpresentasikan sebagai **Note Terenkripsi (Encrypted Note)**.
* Saat berbelanja, note lama dihancurkan (*spent*), merchant menerima pembayaran, dan sisa saldo otomatis di-mint sebagai **Change Note baru** ke Merkle tree.
* Saldo pengguna tetap persisten dan dapat dibelanjakan kembali tanpa perlu melakukan deposit ulang.

---

## 2. Struktur `PrivateNoteV1`

Setiap note pribadi memuat 4 komponen data skalar $\mathbb{F}_r$:

```rust
pub struct PrivateNoteV1 {
    pub owner_pk: Fr,   // Public key pemilik (diturunkan dari spending key)
    pub value: u64,     // Nominal integer base units (USDC: 6 desimal, misal 100 USDC = 100_000_000)
    pub blinding: Fr,   // Faktor acak persembunyian komitmen (blinding randomness)
    pub asset_id: Fr,   // Pengenal token (misal: hash alamat kontrak USDC di Arbitrum)
}
```

### Derivasi Kunci Pemilik:
* `sk_owner` $\in \mathbb{F}_r$: Kunci privat pembelanjaan (*spending key*) yang hanya disimpan di wallet pengguna.
* `owner_pk` $= \text{Poseidon}(\text{DOMAIN\_OWNER\_PK}, sk_{owner})$.
* `nullifier_key` $(nk) = \text{Poseidon}(\text{DOMAIN\_NULLIFIER\_KEY}, sk_{owner})$.

---

## 3. Komitmen Note (*Note Commitment*)

Komitmen note adalah *hash* searah yang dipublikasikan ke on-chain Merkle tree. Komitmen menyembunyikan identitas pemilik, nominal saldo, dan jenis aset:

$$\text{Commitment} = \text{Poseidon}_5(\text{DOMAIN\_NOTE\_COMMITMENT}, \text{owner\_pk}, \text{Fr}(value), \text{blinding}, \text{asset\_id})$$

Sifat komitmen:
* **Hiding:** Pihak luar (termasuk relayer dan validator) tidak dapat mengetahui siapa pemilik note atau berapa nominal di dalamnya.
* **Binding:** Pemilik tidak dapat mengubah nominal atau pemilik note tanpa mengubah nilai komitmen.

---

## 4. Derivasi Nullifier (Pencegah Double-Spend)

Agar sebuah note tidak dapat dibelanjakan dua kali, setiap pembelanjaan harus menerbitkan sebuah **Nullifier** unik on-chain:

$$\text{Nullifier} = \text{Poseidon}_3(\text{DOMAIN\_NULLIFIER}, nk, \text{Fr}(leaf\_index))$$

Karakteristik Nullifier Nimbus:
1. **Unlinkable:** Orang lain tidak bisa menghubungkan antara `Commitment` di masa lalu dengan `Nullifier` saat di-spend.
2. **Deterministic per Leaf:** Note yang sama pada posisi daun Merkle tree yang sama selalu menghasilkan nullifier yang identik.
3. **Owner-Bound:** Hanya pihak yang mengetahui `sk_owner` yang bisa menghasilkan nullifier yang sah.

---

## 5. Poseidon Incremental Merkle Tree

* **Kedalaman (*Depth*):** 20 tingkat (`MERKLE_TREE_DEPTH = 20`).
* **Kapasitas:** Mampu menampung hingga $2^{20} = 1.048.576$ private notes per pool.
* **Fungsi Hash:** Poseidon 2-ke-1 (`width = 3`):
  $$\text{Parent} = \text{Poseidon}_3(\text{DOMAIN\_MERKLE\_NODE}, \text{Left}, \text{Right})$$
* **Accepted Root History:** Smart contract menyimpan daftar root terkini untuk memverifikasi keanggotaan note bahkan jika ada note baru yang masuk saat transaksi sedang dalam perjalanan di mempool.

---

## 6. Hukum Konservasi Nilai (*Value Conservation*)

Setiap transaksi private spend wajib memenuhi persamaan kekekalan nilai secara eksak:

$$\text{Input Note Value} = \text{Merchant Payout} + \text{Protocol Fee} + \text{Execution Fee} + \text{Change Note Value}$$

* Semua nominal dihitung menggunakan integer murni 64-bit (`u64`) tanpa floating-point.
* Jika seluruh saldo dibelanjakan habis ($\text{Change Value} = 0$), maka flag `has_change = 0` dan tidak ada change note yang dibuat di Merkle tree.
