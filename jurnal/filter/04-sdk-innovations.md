# Inovasi Jurnal untuk Client SDK & Wallet State (`nimbus-sdk`)

Dokumen ini memetakan paper terpilih untuk arsitektur client SDK dan manajemen wallet state Nimbus ([`nimbus-sdk`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/)), mencakup [`note_wallet.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/wallet/note_wallet.rs), pembuktian ZK klien (WASM), penemuan note (*scanning*), dan stealth address.

---

## 1. Oblivious Note Synchronization (Sean Bowe & Ian Miers — IACR 2025/2031 & Starknet 2026/463)
* **File Jurnal:** [`jurnal/Note-on-Notes-Scalable-Anonymous-Payments.md`](file:///workspaces/Zeltra-Protocol/jurnal/Note-on-Notes-Scalable-Anonymous-Payments.md) & [`jurnal/Scalable-Compliant-Privacy-Starknet.md`](file:///workspaces/Zeltra-Protocol/jurnal/Scalable-Compliant-Privacy-Starknet.md)
* **Problem di Sistem Klasik:**
  Light client (browser/mobile) yang ingin mengetahui saldo note miliknya terpaksa mengunduh seluruh riwayat ciphertext on-chain dan melakukan *trial decryption* satu per satu. Hal ini menguras baterai ponsel, menghabiskan kuota data, dan membutuhkan waktu sinkronisasi hingga beberapa menit. Jika klien bertanya langsung ke relayer, privasi alamat terbongkar seketika.
* **Inovasi Paper:**
  *Oblivious Synchronization*: Klien menggunakan blinding tag atau kueri Threshold OPRF ke relayer. Relayer mengembalikan kandidat note milik klien tanpa pernah mengetahui identitas penerima atau nilai transaksinya. Menghemat beban scanning hingga 99.6%.
* **Implementasi di Nimbus SDK:**
  * Komponen: [`nimbus-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/wallet/note_wallet.rs) dan [`nimbus-sdk/src/zk_wasm.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/zk_wasm.rs).
  * Method sinkronisasi baru: `sync_oblivious(relayer_client)` yang memungkinkan wallet berbasis web/WASM memulihkan saldo instan dalam <500 milidetik.

---

## 2. zkBSA: Auditable & Compliant Stealth Addresses (IACR ePrint 2026/513)
* **File Jurnal:** [`jurnal/zkBSA-Auditable-Compliant-Stealth-Addresses-Blockchains.md`](file:///workspaces/Zeltra-Protocol/jurnal/zkBSA-Auditable-Compliant-Stealth-Addresses-Blockchains.md)
* **Problem di Sistem Klasik:**
  Stealth address konvensional (ERC-5564) menghasilkan alamat satu kali pakai yang sepenuhnya terputus dari regulasi perbankan/AML, sehingga bursa terpusat (CEX) sering menolak deposit yang berasal dari stealth address.
* **Inovasi Paper:**
  Pengirim dapat menghasilkan alamat tersembunyi (stealth address) yang disertai bukti ZK bahwa alamat penerima terhubung dengan kredensial KYC valid, tanpa membocorkan identitas penerima di publik ledger.
* **Implementasi di Nimbus SDK:**
  * Method baru di wallet: `send_to_stealth_compliant(recipient_meta, kyc_attestation)`.
  * Memungkinkan transfer antar-wallet di Nimbus diterima secara sah oleh platform kepatuhan tanpa melanggar privasi visual di block explorer Arbitrum.

---

## 3. Secure Hierarchical Deterministic Wallets with Stealth Addresses (IACR ePrint 2022/627)
* **File Jurnal:** [`jurnal/Secure-Hierarchical-Deterministic-Wallet-Stealth-Address.md`](file:///workspaces/Zeltra-Protocol/jurnal/Secure-Hierarchical-Deterministic-Wallet-Stealth-Address.md)
* **Problem di Sistem Klasik:**
  Derivasi kunci deterministik (BIP-32) jika dikombinasikan secara naif dengan stealth address dapat membocorkan *master private key* jika salah satu *ephemeral private key* anak terekspos (*hardened vs non-hardened vulnerability*).
* **Inovasi Paper:**
  Struktur pembagian level derivasi kunci asimetris: pemisahan yang terbukti aman antara *spending key* ($sk$), *nullifier key* ($nk$), dan *viewing key* ($vk$).
* **Implementasi di Nimbus SDK:**
  * Memvalidasi dan memperkuat derivasi kunci master di [`nimbus-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/wallet/note_wallet.rs) (`from_seed`) agar tahan terhadap serangan kebocoran child key.

---

## 4. PriSrv: Private Service Discovery for Anonymous Clients (IACR ePrint 2024/1783)
* **File Jurnal:** [`jurnal/PriSrv-Private-Service-Discovery.md`](file:///workspaces/Zeltra-Protocol/jurnal/PriSrv-Private-Service-Discovery.md)
* **Problem di Sistem Klasik:**
  Klien SDK yang mencari relayer dengan fee terendah atau rute tercepat sering kali membocorkan IP dan karakteristik transaksinya ke jaringan relayer melalui query discovery publik.
* **Inovasi Paper:**
  Protokol gossip terenkripsi dengan *Private Information Retrieval (PIR)* ringan, memungkinkan SDK memilih relayer optimal secara anonim.
* **Implementasi di Nimbus SDK:**
  * Modul relayer selection di SDK: Klien dapat membandingkan quote fee relayer secara private tanpa metadata profiling.

---

## 5. MinMandate: Private Task-Scoped Payment Authorization for Adaptive Agent Workflows (2026)
* **File Jurnal:** [`jurnal/MinMandate-Private-Task-Scoped-Payment.md`](file:///workspaces/Zeltra-Protocol/jurnal/MinMandate-Private-Task-Scoped-Payment.md)
* **Penulis:** Ge Gao, Haining Yu, Zhichao Liu, Dongyang Zhan, Yuanxiao Zhu, Zhongyun Hua
* **Problem di Sistem Klasik:**
  Framework agentic payment yang ada (ERC-4337 paymaster, EIP-7702 delegation) mengizinkan AI agent untuk spend atas nama user, tetapi otorisasi bersifat open-ended: agent diberi budget maksimum dan bebas menggunakannya untuk merchant atau layanan apapun. User tidak bisa membatasi "hanya boleh bayar untuk X, bukan Y" tanpa menulis smart contract custom.
* **Inovasi Paper:**
  *Scoped Credential* berbasis constraint: User menerbitkan credential yang hanya valid untuk kategori pengeluaran spesifik (misal: "hanya untuk compute API calls", "hanya untuk domain tertentu", "maksimum 3 kali per hari"). Merchant yang menerima pembayaran harus membuktikan dirinya masuk kategori yang diizinkan via ZK proof of category membership — tanpa mengungkap kategori lain yang diizinkan.
* **Implementasi di Nimbus SDK:**
  * Method baru di SDK: `authorize_agent(scope: SpendScope, max_amount: u64, expiry: u64) -> ScopedCredential`.
  * `SpendScope` adalah enum yang di-encode ke dalam BLS credential saat issuance, diverifikasi di contract via tambahan gadget circuit pada `PrivateNoteCircuit`.
  * Use case langsung: AI agent yang mengelola micro-payment untuk API calls (GPT, compute, storage) tidak bisa di-exploit untuk drain wallet ke merchant arbitrary — scope constraint di-enforce secara kriptografis on-chain, bukan hanya di level aplikasi.
  * Komponen: [`nimbus-sdk/src/wallet/agent_wallet.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/wallet/agent_wallet.rs) (baru), integrate dengan `DelegProof-EIP-7702` untuk gasless execution.
