# Review Konsistensi Public Inputs ZK: Kontrak, Core, dan Relayer

> Catatan untuk pemeriksaan manual, bukan patch atau sertifikasi keamanan.
> PR ini hanya menambah dokumen; tidak mengubah circuit, kontrak, relayer,
> storage, VK, kebijakan fee, atau deployment.

## 1. Ruang Lingkup dan Kesimpulan

- Repo: `0xReLogic/Zeltra-Protocol` (nama engine di source: Nimbus).
- Revision yang diperiksa: [`de740d464461e47f87941e09c530ef03f7097b52`][revision].
- Tanggal review: 7 Oktober 2026.
- Metode: pembacaan source dan keputusan desain, menjalankan test pembangkit
  VK yang sudah ada, lalu membandingkan konstanta VK dengan source kontrak.
- CodeGraph tidak tersedia di VM maupun integrasi MCP sesi ini, sehingga
  penelusuran dilakukan langsung pada source. Tidak ada perubahan konfigurasi
  untuk mengatasi keterbatasan tersebut.
- Tidak menjalankan transaksi testnet/mainnet, reproduksi serangan, full test
  suite, audit formal, atau pemeriksaan bytecode deployment.

**Kesimpulan utama:** perbandingan “kontrak 12/13 input versus core 4 input”
mencampur **dua circuit yang berbeda**. Jalur private-note memakai
`PrivateNoteCircuit` dengan 12 public inputs dan verifier dengan 13 titik IC.
Jalur compliance memakai `ComplianceCircuit` dengan 4 public inputs dan
verifier dengan 5 titik IC. Tidak ditemukan ketimpangan jumlah input di kedua
pasangan tersebut pada revision ini.

Namun, konsistensi jumlah input **tidak sama dengan kelengkapan statement
kriptografis atau keamanan seluruh pipeline**. Empat poin review di bagian 5
perlu diperiksa terpisah.

## 2. Dua Circuit, Dua Statement

| Aspek | Private note | Compliance |
| --- | --- | --- |
| Core | `PrivateNoteCircuit`, [`note_circuit.rs`][note-struct] | `ComplianceCircuit`, [`compliance_circuit.rs`][compliance-constraints] |
| Public inputs | 12 | 4: `root`, `nullifier`, `recipient`, `amount` |
| Verifier kontrak | [`groth16_note_verifier.rs`][note-verifier] | [`verification.rs`][compliance-verifier] |
| Titik IC dalam VK | 13: `IC[0]..IC[12]` | 5: `IC_0..IC_4` |
| Entry point | `spend_private_note` melalui `_spend_private_note` | `verify_compliance` melalui `_verify_compliance` |
| Statement yang tampak di source | Note membership, nullifier, change commitment, konservasi nilai dan batas 64-bit | Pengetahuan `secret` dan `randomness` sehingga `nullifier = Poseidon(secret, randomness)` |
| Status hubungan ke pembayaran | Proof diverifikasi sebelum effects dan transfer | Fungsi view terpisah; tidak ditemukan panggilan dari jalur spend di source kontrak yang diperiksa |

Untuk Groth16, verifier menghitung:

```text
L = IC[0] + sum(public_input[i] * IC[i + 1])
```

`IC[0]` adalah suku konstan, **bukan input transaksi tambahan**.
Jadi 13 titik IC berarti 12 public inputs, bukan 13 public inputs.
Hal serupa berlaku untuk 5 titik IC pada compliance circuit.

## 3. Apa Saja 12 Public Inputs Itu?

Public inputs adalah parameter statement pembayaran, **bukan 12 produk
atau 12 merchant**. Urutan alokasi core, ekstraksi untuk verifier off-chain,
dan penyusunan kontrak cocok:

- [Alokasi circuit][note-allocation].
- [Ekstraksi public inputs][note-extraction].
- [Penyusunan input dan verifikasi kontrak][contract-note-inputs].

| Indeks | Input | Peran | Sumber di kontrak |
| --- | --- | --- | --- |
| 0 | `note_root` | Root accumulator tempat input note dibuktikan menjadi anggota | Parameter; harus tercatat pada `accepted_note_roots` |
| 1 | `input_nullifier` | Identitas spend untuk mencegah pemakaian ulang note | Parameter; diperiksa pada `note_nullifiers` |
| 2 | `output_commitment` | Komitmen change note | Parameter; nol jika tidak ada change |
| 3 | `recipient` | Alamat penerima payout | Parameter address, dipad menjadi 32 byte |
| 4 | `merchant_amount` | Nilai payout ke penerima | Parameter |
| 5 | `protocol_fee` | Nilai fee protokol dalam transaksi | Parameter |
| 6 | `execution_fee` | Nilai fee eksekusi relayer | Parameter |
| 7 | `quote_hash` | Identifier/digest quote yang menjadi bagian statement | Parameter; circuit tidak merekonstruksi seluruh quote |
| 8 | `chain_id` | Domain chain | `env_chain_id()`, bukan nilai bebas dari pemanggil |
| 9 | `contract_address` | Domain kontrak | `env_contract_address()`, bukan nilai bebas dari pemanggil |
| 10 | `expiry` | Batas waktu transaksi | Parameter; pengecekan waktu dilakukan kontrak |
| 11 | `has_change` | Flag keberadaan change note (0 atau 1) | Parameter; dibatasi boolean di circuit dan kontrak |

**Nominal:** private-note circuit memproses lima nilai:

```text
input_value = merchant_amount + protocol_fee + execution_fee + change_value
```

`input_value` dan `change_value` adalah private witnesses; tiga nominal
pembayaran/fee adalah public inputs. Kelimanya diberi range constraint
`[0, 2^64)` dan persamaan konservasi nilai di-[enforce][note-accounting].
Ini bukan circuit yang hanya memproses satu `amount`.

**Merchant dan storage:** [`storage.rs`][storage] tidak menyimpan profil merchant
kompleks. Ia menyimpan liability, accumulator, registry root, dan nullifier,
antara lain. [`PrivateNoteSpend`][event] merekam `recipient` dan
`merchant_amount` sebagai event transaksi. Kehadiran nama merchant pada event
tidak mengharuskan semua field event menjadi storage atau public input circuit.
Jalur ini mendukung satu input note, satu penerima payout, dan satu change note
per proof; bukan multi-merchant dalam satu proof.

## 4. Pemeriksaan Verifying Key (VK)

Test existing yang dijalankan dari root workspace:

```sh
cargo test -p nimbus-core --release --lib -- \
  --nocapture --include-ignored print_ evm_vk
```

Hasil: **3 passed, 0 failed, 114 filtered out**:

1. `compliance_circuit::tests::test_evm_vk_constants_generation`
2. `compliance_circuit::tests::print_evm_vk_hex`
3. `note_circuit::tests::print_note_circuit_evm_vk_hex`

Output hex pembangkit VK kemudian dibandingkan dengan konstanta source:

| Pasangan | Komponen yang diperiksa | Hasil pada revision review |
| --- | --- | --- |
| Compliance | `alpha`, `beta`, `gamma`, `delta`, 5 IC | Seluruh 9 titik cocok byte-per-byte dengan `verification.rs` |
| Private note | `alpha`, `beta`, `gamma`, `delta`, 13 IC | Seluruh 17 titik cocok byte-per-byte dengan `groth16_note_verifier.rs` |

Cara memeriksa ulang: jalankan masing-masing print test secara terpisah agar
output tidak bercampur, kemudian cocokkan komponen berdasarkan nama/indeks
dan encoding EVM-nya, bukan hanya menghitung IC:

```sh
cargo test -p nimbus-core --release --lib \
  compliance_circuit::tests::print_evm_vk_hex -- --exact --nocapture
cargo test -p nimbus-core --release --lib \
  note_circuit::tests::print_note_circuit_evm_vk_hex -- --exact --nocapture
```

**Batasan:** test pembangkit tidak otomatis menegaskan kecocokan dengan
konstanta kontrak; perbandingan di atas dilakukan terpisah. Tidak ditemukan
assertion lintas core/kontrak untuk kecocokan VK pada source yang ditelusuri.
VK yang cocok juga bukan bukti ceremony aman: source masih menggunakan
deterministic Phase A setup dengan seed publik. Kelayakan production/MPC
ceremony tetap pekerjaan terpisah sebagaimana backlog repo.

## 5. Poin yang Perlu Review Detail

Label berikut membedakan fakta source dari dampak yang belum direproduksi.
Urutan ini bukan penetapan severity final.

### R1 — Statement Compliance Tidak Membuktikan Association Membership

**Fakta terverifikasi:** [alokasi dan constraints][compliance-constraints]
menunjukkan `root`, `recipient`, dan `amount` dialokasikan tetapi tidak dipakai
dalam constraint. Relasi yang di-enforce adalah nullifier dari dua witness.
Tidak ada witness Merkle path, pengecekan membership, range check amount,
atau hubungan witness tersebut dengan credential/note yang diterbitkan.

Di kontrak, [`_verify_compliance`][compliance-entry] memastikan root terdaftar,
kemudian memverifikasi proof. Registry root yang valid tidak membuktikan
bahwa witness pemegang proof merupakan anggota root tersebut.

**Kesenjangan:** docstring core mengklaim batas nominal, validitas recipient,
dan root; docstring kontrak menyebut membership di dalam circuit. Klaim itu
lebih luas daripada statement implementasi.

**Batas dampak:** ini bukan bukti bypass `PrivateNoteCircuit` atau pencurian
saldo. `verify_compliance` adalah view terpisah. Risiko meningkat bila penerima
atau integrasi memperlakukan hasil `true` sebagai bukti asal dana bersih.
[DEC-026][dec26] sendiri memisahkan validity dari receiver policy dan menaruh
receiver-bound policy pada fase berikutnya.

**Untuk diperiksa manual:**

- [ ] Tetapkan statement compliance yang sebenarnya diinginkan; jangan sekadar menambah 8 input supaya jumlahnya sama dengan note circuit.
- [ ] Identifikasi integrasi eksternal yang mengandalkan `verify_compliance`.
- [ ] Uji root berbeda, amount berbeda, dan recipient berbeda pada proof yang sama; catat perilaku aktual verifier.
- [ ] Uji witness yang tidak memiliki membership/credential sah dan pastikan desain target menolaknya.
- [ ] Putuskan apakah endpoint dipertahankan sebagai prototype, dinonaktifkan, atau diganti dengan statement lengkap sebelum dipasarkan sebagai proof of innocence.

### R2 — Umur Root Bukan Bukti Lama Kepemilikan Dana

**Fakta terverifikasi:** [spend BLS][legacy-root-fee] dan
[batch BLS][batch-root-fee] memilih 40 bps alih-alih 45 bps ketika root
terdaftar telah berumur minimal 30 hari. Root berasal dari parameter caller.
[Pesan BLS `compute_spend_hash`][spend-hash] tidak memasukkan root.
Tidak ditemukan pembuktian membership pemilik credential pada root yang
dipakai memilih tier fee di jalur tersebut.

**Interpretasi yang perlu diputuskan:** apabila diskon dimaksudkan untuk
pengguna yang menahan dana 30 hari, umur registry root tidak membuktikan
umur dana pengguna. Jika kebijakan memang diskon global berdasarkan umur
root, perilaku ini bisa sesuai desain; narasi “holding-time discount” perlu
disesuaikan. Pemilihan root bukan bypass BLS: signature dan pemeriksaan lain
tetap harus valid.

**Batas dampak:** kandidat gap kebijakan fee, bukan bukti mint atau double
spend. Pemanfaatan membutuhkan root nonzero yang sudah terdaftar dan cukup
tua serta spend yang memenuhi pemeriksaan lain. Dampak finansial belum diukur.

**Status Resolusi (DEC-028):**
- [x] **Diputuskan:** Eligibility diskon umur root dihapus permanen. Fee protokol disatukan menjadi **Flat 45 bps (0.45%)** deterministik untuk seluruh spend (DEC-028).
- [x] Percabangan `delta_t >= 30 days` di contract `spend()` dan `batch_spend()` dihapus, menghemat SLOAD gas dan menutup celah arbitrase root tua.
- [x] `resolve_fee_tier` dan `root_timestamp_cache` di relayer quote handler dibersihkan.

### R3 — Domain Preflight Relayer Tidak Dibandingkan dengan Target Eksekusi

**Fakta terverifikasi:** [handler][relayer-inputs] membandingkan indeks
0–7, 10, dan 11 dengan payload, tetapi tidak membandingkan indeks 8
(`chain_id`) dan 9 (`contract_address`) dengan domain target relayer.
[Local Groth16 preflight][relayer-preflight] menggunakan vector dari caller.
Pemeriksaan domain EIP-712 quote, bila dijalankan, tidak menggantikan
perbandingan kedua indeks Groth16 tersebut.

Kontrak justru [menyusun domain dari runtime][contract-note-inputs].
Proof valid yang dibuat untuk domain berbeda dapat valid secara lokal
terhadap vector caller tetapi tidak valid terhadap vector kontrak target.
Mengubah domain pada proof lama tidak sama dengan membuat proof baru untuk
domain lain: public-input binding pada note circuit tetap relevan.

[Dispatcher][dispatcher] meneruskan transaksi ke
[`broadcast_spend_private_note_transaction`][broadcast]. Fungsi broadcast
menetapkan gas limit lalu mengirim transaksi; tidak terlihat dry-run
`eth_call` terhadap calldata spend pada jalur yang ditelusuri.

**Status Resolusi (DEC-028):**
- [x] **Selesai Diimplementasikan:** Indeks 8 (`chain_id`) dan 9 (`contract_address`) divalidasi secara ketat terhadap domain target `state.evm_client` di relayer sebelum enqueue (`spend.rs`).
- [x] Proof dengan domain chain_id atau contract_address berbeda langsung ditolak dengan `HTTP 400 REJECTED` di CPU lokal relayer tanpa broadcast on-chain (mencegah gas-griefing).
- [x] Unit test `test_handle_private_note_spend_reject_domain_mismatch_chain_id` dan `contract_address` ditambahkan dan pass 100%.

### R4 — Vector Public Inputs Kosong Melewati Local Proof Preflight

**Fakta tambahan saat menyiapkan catatan:** [`public_inputs` DTO][dto]
memiliki default vector kosong. Seluruh canonicality check, semantic binding,
dan verifikasi Groth16 lokal [berada di dalam][relayer-inputs]
`if !payload.public_inputs.is_empty()`.

Jadi input kosong/field tidak disertakan melewati blok preflight tersebut,
bukan ditolak karena tidak berjumlah 12. [Enqueue][enqueue] berada setelah
blok itu. Ini mengoreksi penyederhanaan “relayer selalu mewajibkan 12 input”:
**panjang 12 hanya diwajibkan jika vector tidak kosong**.

**Status Resolusi (DEC-028):**
- [x] **Selesai Diimplementasikan:** Klausa bypass `if !payload.public_inputs.is_empty()` dihapus total.
- [x] Relayer menegakkan aturan fail-closed: `payload.public_inputs.len() == 12` wajib. Input kosong `[]` atau panjang tidak sesuai langsung ditolak `HTTP 400 REJECTED` sebelum enqueue.
- [x] Unit test `test_handle_private_note_spend_reject_empty_public_inputs` dan `wrong_public_inputs_count` ditambahkan dan pass 100%.


## 6. Klarifikasi atas Observasi `_binding`

Di [note circuit][note-binding], hash scope dan `_binding` dihitung tanpa
output digest dibandingkan ke nilai lain. **Ini tidak cukup untuk menyatakan
hash tersebut “sia-sia” atau public inputs tidak terikat.** Gadget Poseidon
menggunakan operasi `FpVar` yang menghasilkan constraints; [S-box][poseidon]
mengandung perkalian variabel.

Ada test existing untuk tampered recipient serta pertukaran
`chain_id`/`contract_address` yang mengharapkan verification gagal. Test
tersebut tidak dijalankan dalam pemeriksaan VK di bagian 4.
Review constraint/VK dan negative tests diperlukan sebelum menghapus gadget
atau mengganti circuit. Mengubah circuit juga membutuhkan regenerasi
proving/verifying key dan sinkronisasi verifier.

Header bagian `9.` ganda adalah masalah editorial kecil, bukan bukti celah
kriptografi. Tidak diperbaiki dalam PR ini.

## 7. Batas Kesimpulan dan Keputusan Pemilik

1. Jumlah public inputs dan VK cocok pada source revision ini; itu tidak
   membuktikan seluruh protocol production-ready.
2. Membership `note_root` pada accumulator transaksi berbeda dengan membership
   association/policy root. Jangan mencampurkan keduanya dalam klaim compliance.
3. Kesimpulan fee berlaku pada jalur **legacy BLS/batch**, bukan otomatis pada
   seluruh private-note fee policy.
4. Risiko relayer belum dibuktikan lewat transaksi gagal berbayar; tidak ada
   klaim kerugian aktual atau bypass settlement.
5. Dokumen ini tidak menentukan patch maupun mengganti keputusan desain.
   Pemilik dapat memeriksa checkbox di atas dan memilih tindak lanjut terpisah.

## Referensi Source (Dipin ke Revision Review)

[revision]: https://github.com/0xReLogic/Zeltra-Protocol/commit/de740d464461e47f87941e09c530ef03f7097b52
[note-struct]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-core/src/note_circuit.rs#L20-L64
[note-allocation]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-core/src/note_circuit.rs#L136-L150
[note-extraction]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-core/src/note_circuit.rs#L555-L570
[note-accounting]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-core/src/note_circuit.rs#L247-L308
[note-binding]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-core/src/note_circuit.rs#L310-L335
[note-verifier]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/groth16_note_verifier.rs#L16-L90
[compliance-constraints]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-core/src/compliance_circuit.rs#L1-L88
[compliance-verifier]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/verification.rs#L152-L197
[compliance-entry]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/verification.rs#L298-L342
[contract-note-inputs]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/spend.rs#L665-L724
[storage]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/storage.rs#L5-L104
[event]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/events.rs#L61-L70
[legacy-root-fee]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/spend.rs#L173-L225
[batch-root-fee]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/lib.rs#L649-L695
[spend-hash]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-contracts/src/helpers.rs#L101-L119
[relayer-inputs]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-node/src/handlers/spend.rs#L587-L785
[relayer-preflight]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-node/src/handlers/spend.rs#L743-L785
[dto]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-node/src/dto.rs#L193-L203
[enqueue]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-node/src/handlers/spend.rs#L901-L946
[dispatcher]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-node/src/handlers/spend.rs#L1221-L1271
[broadcast]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-node/src/evm_client.rs#L895-L929
[poseidon]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/nimbus-core/src/poseidon.rs#L195-L200
[dec26]: https://github.com/0xReLogic/Zeltra-Protocol/blob/de740d464461e47f87941e09c530ef03f7097b52/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md
