### 1\. Solusi Entry Privacy (Shielded Deposit / Ephemeral Wallet)

* **Masalah**: Event `DepositFee` di-chain mencatat alamat pengirim dan nominal deposit secara publik, sehingga mudah dikorelasikan[1].
* **Solusi**: Integrasikan pembuatan *stealth address* atau *ephemeral keypair* (alamat sekali pakai) di level SDK *client* sebelum melakukan deposit[2][3].
* **Hasil**: Event `DepositFee` publik hanya mencatat alamat sementara yang tidak terikat ke *wallet* utama pengguna[2].

### 2\. Solusi Exit Privacy (Masking Alamat Penerima / ERC-5564)

* **Masalah**: Alamat penerima (*merchant payout*) pada fungsi *spend* bersifat publik, memicu risiko *merchant clustering* oleh analis *on-chain*[4][5].
* **Solusi**: Terapkan skema pengiriman ke *one-time stealth address* (ERC-5564 / ZK-Stealth) untuk penerima saat eksekusi *spend*[3].
* **Hasil**: Alamat penerima asli tersembunyi, sehingga transaksi tidak bisa dikelompokkan berdasarkan alamat *merchant*[5].

### 3\. Mencegah Pola Angka (Anti Value-Clustering)

* **Masalah**: Pola nominal deposit dan *spend* yang identik/mirip bisa dipakai untuk melacak keterhubungan transaksi[6].
* **Solusi**: Sisipkan pemecahan *note* acak atau *dummy change note* otomatis di Merkle tree (*LeanIMT*) saat proses pembuatan *change note*[7][8].
* **Hasil**: Pola nilai transaksi teracak secara statistik di dalam *pool*.

### 4\. Penguatan Trust Model Guardian (Mendekati Trustless)

* **Masalah**: Keamanan saat ini masih mengandalkan konsensus 3-dari-5 *quorum Guardian* dan ketergantungan pada *Leader node*[9].
* **Solusi**:
  * Perbesar *quorum* (misalnya dari 3/5 menjadi 5/9 atau 7/13)[3].
  * Terapkan mekanisme *Staking &amp; Slashing* (penalti sita dana jika *Guardian* mencoba berkolusi)[3][9].
  * Gunakan skema *Leaderless Threshold BLS* agar tidak ada *single point of failure* jika *Leader node* offline[3].
* **Hasil**: *Trust model* jauh lebih terdesentralisasi dan aman dari kecurangan internal.