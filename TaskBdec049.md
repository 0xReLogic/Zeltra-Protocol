# Task B: Integrasi Client SDK, Negative Test Matrix & Verifikasi Dudect (DEC-049 Fase 2)

> **Fokus Sesi 2:** Integrasi tingkat tinggi pada client sync scanner di `zeltra-sdk`, implementasi matriks uji adversarial (Gate V-1 s/d Gate V-4), dan pengujian waktu diferensial.  
> **Status:** Pending Execution (Menunggu Selesainya Task A)  
> **Referensi Arsitektur:** [`research/decisions/DEC-049-leakage-resilient-decapsulation-and-constant-time-flag-defense.md`](research/decisions/DEC-049-leakage-resilient-decapsulation-and-constant-time-flag-defense.md)

---

##  Target Utama
1. Mengintegrasikan `ConstantTimeMemoScanner` ke dalam wallet sync worker di `zeltra-sdk/src/wallet/note_wallet/sync.rs`.
2. Mengimplementasikan Gate V-1 (Known-Answer Tests) dan Gate V-2 (5 Negative Test Categories).
3. Mengonfigurasi benchmark timing diferensial (`dudect`) dengan kriteria statistik $|t| < 4.5$.
4. Memvalidasi ketiadaan instruksi lompatan bersyarat (`jne`, `je`, `jmp`) pada binary terkompilasi (Gate V-4).

---

##  Checklist Pekerjaan

- [ ] **B1: Integrasi SDK Scanner (`zeltra-sdk`)**
  - Pasang scanner CT-FOE pada loop trial-decryption log event MMR (`NoteCommitmentAppended`).
  - Pastikan branch sync client tidak membocorkan timing kegagalan dekripsi ke thread lain.
  - Periksa kompatibilitas dengan WebAssembly Worker (`DEC-041`).

- [ ] **B2: Suite Uji Adversarial (Gate V-2 Negative Matrix)**
  - Uji Kategori 1: Ephemeral public key non-contributory (titik orde rendah).
  - Uji Kategori 2: View Tag cacat / tidak cocok.
  - Uji Kategori 3: Nonce transport tampered.
  - Uji Kategori 4: Committing tag HMAC corrupt / forged.
  - Uji Kategori 5: Skalar $\rho$ non-kanonikal ($\ge r$).
  - Pastikan 100% kasus gagal secara *fail-closed* tanpa kebocoran alokasi memori.

- [ ] **B3: Pengujian Waktu Diferensial (Gate V-3 `dudect`)**
  - Siapkan harness pengujian statistik waktu: $N \ge 10^6$ trace untuk memo valid vs memo acak.
  - Jalankan evaluasi *Welch's two-tailed t-test*.
  - Verifikasi kriteria lolos: **$|t| < 4.5$**.
  - Jalankan kontrol negatif (acak vs acak) dan kontrol positif (bocoran buatan) untuk validasi harness.

- [ ] **B4: Verifikasi Assembly & CI Alignment (Gate V-4)**
  - Dump assembly dari target binary: `objdump -d` / `cargo asm`.
  - Verifikasi ketiadaan instruksi lompatan rahasia pada alur `trial_decrypt_ct`.
  - Update checklist backlog [`todo.md`](todo.md) dan perbarui status audit `AUD-25`, `AUD-26`, `AUD-27` menjadi `VERIFIED`.

---

##  Kriteria Sukses Sesi 2
- Matriks uji negatif Gate V-2 lolos 100%.
- Pengujian statistik `dudect` mencatat $|t| < 4.5$ tanpa false positive.
- Sinkronisasi dompet `zeltra-sdk` berjalan mulus dan kebal terhadap Flag DPA serta timing leakage.
