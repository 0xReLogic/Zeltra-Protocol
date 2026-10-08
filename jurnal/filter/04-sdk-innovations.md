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
* **File Jurnal:** [`jurnal/honorable/zkBSA-Auditable-Compliant-Stealth-Addresses-Blockchains.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/zkBSA-Auditable-Compliant-Stealth-Addresses-Blockchains.md)
* **Penulis:** Siyuan Zheng, Zhe Han (Maret 2026, Ant International)
* **Problem di Sistem Klasik:**
  Stealth address konvensional (ERC-5564) menghasilkan alamat satu kali pakai yang sepenuhnya terputus dari regulasi perbankan/AML, sehingga bursa terpusat (CEX) sering menolak deposit yang berasal dari stealth address karena tidak bisa diaudit.
* **Inovasi Paper:**
  Framework modular menggabungkan ERC-5564 stealth address, enkripsi kunci publik EC-ElGamal, komitmen whitelist, dan ZK proofs. Pengirim dapat membuktikan on-chain bahwa transaksi ditujukan ke penerima yang sah/terdaftar whitelist tanpa membuka identitas penerima kepada publik. Evaluasi menunjukkan pembuktian ZK selesai dalam <5.3 detik dan verifikasi on-chain tetap konstan hemat (~235k gas).
* **Implementasi di Nimbus SDK:**
  * Method baru di wallet: `send_to_stealth_compliant(recipient_meta, kyc_attestation)`.
  * Memungkinkan deposit/transfer di Nimbus diterima secara sah oleh platform kepatuhan/CEX tanpa melanggar privasi penerima di Arbiscan. Resolusi tuntas untuk masalah *"The Glass Door Problem"* di Fase 3.

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

---

## 6. zkTLS Invoice Attestation for Autonomous Agents (IACR ePrint 2026/199 — Pivot dari zkAgent)
* **File Jurnal:** [`jurnal/zkAgent-Verifiable-LLM-Agent-Execution-One-Shot-Transcript-Proofs.md`](file:///workspaces/Zeltra-Protocol/jurnal/zkAgent-Verifiable-LLM-Agent-Execution-One-Shot-Transcript-Proofs.md)
* **Problem di Sistem Klasik:**
  Mengeksekusi pembuktian ZK secara penuh atas seluruh inferensi LLM (*full model execution trace*) untuk setiap transaksi mikro agen AI ($0.05) terbukti tidak layak secara komputasi (*computationally infeasible*), menghabiskan gigabyte RAM dan waktu pembuktian bermenit-menit.
* **Inovasi Paper & Pivot:**
  Pivot dari model inferensi penuh ke *zkTLS Invoice Attestation*: SDK membuktikan secara kriptografis bahwa agen AI menerima faktur penagihan yang sah dari endpoint merchant/API web (misal Stripe, AWS Billing, OpenAI API) melalui sesi TLS terautentikasi tanpa membocorkan kunci API atau data rahasia pengguna.
* **Implementasi di Nimbus SDK:**
  * Komponen: `nimbus-sdk/src/agent.rs` dan modul kepatuhan `nimbus-sdk/src/compliance/`.
  * Memungkinkan agen AI mengikat pembayaran Note UTXO secara atomik ke komitmen bukti zkTLS faktur web, mencegah *ghost billing* atau pengeluaran fiktif.

---

## 7. Coral-CFG: Structured Business Data Parsing in Client SDK (IACR ePrint 2025/1420)
* **File Jurnal:** [`jurnal/Coral-CFG-Proofs.md`](file:///workspaces/Zeltra-Protocol/jurnal/Coral-CFG-Proofs.md)
* **Problem di Sistem Klasik:**
  Mengekstrak dan memverifikasi field tertentu dari payload terstruktur bisnis (JSON, TOML, format faktur B2B) di dalam sirkuit ZK konvensional membutuhkan puluhan ribu R1CS constraints yang membuat klien browser/ponsel mengalami crash memori.
* **Inovasi Paper:**
  Parser ZK berbasis tata bahasa bebas konteks (CFG) menggunakan pohon biner Left-Child Right-Sibling (LCRS) dan *Segmented Memory (Nebula)*. Mampu memparsing dan membuktikan field nilai uang, mata uang, dan status pembayaran dalam waktu 1–3 detik CPU lokal klien.
* **Implementasi di Nimbus SDK:**
  * Komponen: `nimbus-sdk/src/compliance/cfg.rs`.
  * Mengekstrak komitmen faktur bisnis secara 100% lokal di SDK tanpa membocorkan rincian invoice ke relayer atau validator blockchain, kompatibel langsung dengan spend note ZK-UTXO.

---

## 8. Device-Binding Anonymous Credentials for Mobile Wallets (IACR ePrint 2026/965)
* **File Jurnal:** [`jurnal/Device-Binding-Anonymous-Credentials-Legacy-Phones.md`](file:///workspaces/Zeltra-Protocol/jurnal/Device-Binding-Anonymous-Credentials-Legacy-Phones.md)
* **Problem di Sistem Klasik:**
  Prinsip Nimbus menetapkan penyimpanan note ZK-UTXO 100% di sisi klien (zeroized on drop). Ancaman terbesarnya adalah malware infostealer yang menyalin note dari memori perangkat lunak. Namun, chip hardware Secure Enclave / TEE di miliaran ponsel pintar iOS & Android terkunci secara paten pada kurva ECDSA P-256 dan tidak mendukung operasi kurva pairing BLS12-381.
* **Inovasi Paper:**
  Protokol *Proof-of-Possession Bulletproofs (PoP-BP)* di atas kurva representasi T-256 yang memisahkan domain Secure Enclave P-256 dengan BLS12-381:
  - Ukuran bukti sangat ringkas: ~1.47 kB (jauh di bawah limit calldata L2).
  - Waktu pembuktian prover di ponsel modern: ~208 ms; verifikasi: 35–52 ms.
  - Kompatibel penuh dengan kredensial anonim BLS12-381 + BBS+.
* **Implementasi di Nimbus SDK:**
  * Komponen: `nimbus-sdk/src/credential.rs`.
  * Mengikat note belanja ke otorisasi Secure Enclave/biometrik fisik pengguna. Sekalipun file note dicuri dari memori perangkat lunak oleh infostealer, note tersebut secara matematis mustahil dibelanjakan tanpa kunci privat yang tersimpan di dalam hardware Secure Enclave perangkat fisik.

---

## 9. DelegProof: Formal Accountability for Agent Delegation (IACR ePrint 2026/2060)
* **File Jurnal:** [`jurnal/honorable/DelegProof-EIP-7702-Account-Delegation.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/DelegProof-EIP-7702-Account-Delegation.md)
* **Penulis:** Ruifeng Qian, Chengyu Dong, Lingyu Gao, Yuchang Zhang, Zengli Guo (September 2026)
* **Problem di Sistem Klasik:**
  Mekanisme delegasi eksekusi akun (EIP-7702 / ERC-4337) untuk AI Agent rentan terhadap 4 kelas serangan: (1) cross-chain replay serangan authorization chainId-0, (2) front-run initialization, (3) storage confusion lintas re-delegation, dan (4) ERC-1271 substitution attack yang mencapai 63% delegasi berbahaya di mainnet ($2.36M kerugian).
* **Inovasi & Mitigasi Paper:**
  Analisis simbolik formal pertama (Tamarin models) untuk delegasi EIP-7702. Membuktikan 3 mitigasi formal: (a) larangan otorisasi chainId-0 mencegah cross-chain replay, (b) account-bound initialization gate untuk init authorization, dan (c) namespaced storage slots mencegah kebingungan storage.
* **Implementasi di Nimbus SDK:**
  * Komponen: [`nimbus-sdk/src/wallet/agent_wallet.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/wallet/agent_wallet.rs) dan modul delegasi gasless.
  * Menjamin delegasi pembayaran mikro otonom pada agen AI bebas dari eksploitasi cross-chain replay, front-run init, dan penipuan eksekusi kontrak jahat.

---

## 10. ZK-SNARK Stealth Addresses: Hash-Commitment Ownership Proofs (IEEE 2026)
* **Link Paper:** [`https://ieeexplore.ieee.org/document/11676814/`](https://ieeexplore.ieee.org/document/11676814/)
* **Problem di Sistem Klasik:**
  Skema stealth address konvensional (Umbra, ERC-5564) bergantung penuh pada Elliptic Curve Diffie-Hellman (ECDH) untuk derivasi address spending key publik. Ini menciptakan 3 masalah:
  1. Alamat stealth tetap terekspos sebagai address EVM biasa di explorer on-chain.
  2. Penerima harus memiliki native gas (ETH) di address baru tersebut untuk memindahkan dana keluar atau bergantung pada relayer eksternal yang terpisah.
  3. Logika kepemilikan terkunci mati pada spending key ECDSA statis dan tidak dapat diperluas ke kondisi kepemilikan yang dapat diprogram (*programmable predicates*).
* **Inovasi Paper:**
  Menggantikan derivasi address ECDH dengan skema komitmen hash Poseidon yang dibuktikan lewat ZK-SNARK (Groth16):
  - Mengikat rahasia pengirim dan penerima ke dalam *authentication tag* berbasis hash Poseidon yang disimpan di smart contract stealth wallet:
    $$\text{Tag} = \text{Poseidon}(\text{secret}_{\text{sender}}, \text{secret}_{\text{recipient}})$$
  - Kepemilikan dibuktikan dengan menyerahkan Groth16 proof atas pengetahuan kedua rahasia tanpa pernah membukanya ke publik.
  - Mempertahankan ECIES (Elliptic Curve Integrated Encryption Scheme) hanya untuk transport ephemeral key ringan.
  - Mentransformasikan kepemilikan stealth wallet dari sekadar spending key statis menjadi **Programmable ZK Predicate** yang dapat disisipi aturan arbitrer di dalam sirkuit.
* **Implementasi di Nimbus SDK & Core:**
  * Komponen: [`nimbus-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/wallet/note_wallet.rs) (stealth scanning & ephemeral key transport via ECIES) dan [`nimbus-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note_circuit.rs).
  * Use Case Zeltra: **Direct Stealth Push Payments**. Memungkinkan pengguna Zeltra mengirim dana dari private note pool langsung ke pihak luar/merchant yang hanya mempublikasikan *Stealth Meta-Address* (tanpa mengungkap address publik aslinya di Arbiscan). Penerima dapat mengklaim atau membelanjakan dana via relayer gasless Zeltra tanpa pernah memegang saldo gas ETH on-chain.

