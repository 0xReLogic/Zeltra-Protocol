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
