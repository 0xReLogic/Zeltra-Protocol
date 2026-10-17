# Inovasi Jurnal untuk Client SDK & Wallet State (`zeltra-sdk`)

Dokumen ini memetakan paper terpilih untuk arsitektur client SDK dan manajemen wallet state Zeltra ([`zeltra-sdk`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/)), mencakup [`note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs), pembuktian ZK klien (WASM), penemuan note (*scanning*), dan stealth address.

---

## 1. Oblivious Note Synchronization (Sean Bowe & Ian Miers — IACR 2025/2031 & Starknet 2026/463)
* **File Jurnal:** [`jurnal/Note-on-Notes-Scalable-Anonymous-Payments.md`](file:///workspaces/Zeltra-Protocol/jurnal/Note-on-Notes-Scalable-Anonymous-Payments.md) & [`jurnal/Scalable-Compliant-Privacy-Starknet.md`](file:///workspaces/Zeltra-Protocol/jurnal/Scalable-Compliant-Privacy-Starknet.md)
* **Problem di Sistem Klasik:**
  Light client (browser/mobile) yang ingin mengetahui saldo note miliknya terpaksa mengunduh seluruh riwayat ciphertext on-chain dan melakukan *trial decryption* satu per satu. Hal ini menguras baterai ponsel, menghabiskan kuota data, dan membutuhkan waktu sinkronisasi hingga beberapa menit. Jika klien bertanya langsung ke relayer, privasi alamat terbongkar seketika.
* **Inovasi Paper:**
  *Oblivious Synchronization*: Klien menggunakan blinding tag atau kueri Threshold OPRF ke relayer. Relayer mengembalikan kandidat note milik klien tanpa pernah mengetahui identitas penerima atau nilai transaksinya. Menghemat beban scanning hingga 99.6%.
* **Implementasi di Zeltra SDK:**
  * Komponen: [`zeltra-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs) dan [`zeltra-sdk/src/zk_wasm.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/zk_wasm.rs).
  * Method sinkronisasi baru: `sync_oblivious(relayer_client)` yang memungkinkan wallet berbasis web/WASM memulihkan saldo instan dalam <500 milidetik.

---

## 2. zkBSA: Auditable & Compliant Stealth Addresses (IACR ePrint 2026/513)
* **File Jurnal:** [`jurnal/honorable/zkBSA-Auditable-Compliant-Stealth-Addresses-Blockchains.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/zkBSA-Auditable-Compliant-Stealth-Addresses-Blockchains.md)
* **Penulis:** Siyuan Zheng, Zhe Han (Maret 2026, Ant International)
* **Problem di Sistem Klasik:**
  Stealth address konvensional (ERC-5564) menghasilkan alamat satu kali pakai yang sepenuhnya terputus dari regulasi perbankan/AML, sehingga bursa terpusat (CEX) sering menolak deposit yang berasal dari stealth address karena tidak bisa diaudit.
* **Inovasi Paper:**
  Framework modular menggabungkan ERC-5564 stealth address, enkripsi kunci publik EC-ElGamal, komitmen whitelist, dan ZK proofs. Pengirim dapat membuktikan on-chain bahwa transaksi ditujukan ke penerima yang sah/terdaftar whitelist tanpa membuka identitas penerima kepada publik. Evaluasi menunjukkan pembuktian ZK selesai dalam <5.3 detik dan verifikasi on-chain tetap konstan hemat (~235k gas).
* **Implementasi di Zeltra SDK:**
  * Method baru di wallet: `send_to_stealth_compliant(recipient_meta, kyc_attestation)`.
  * Memungkinkan deposit/transfer di Zeltra diterima secara sah oleh platform kepatuhan/CEX tanpa melanggar privasi penerima di Arbiscan. Resolusi tuntas untuk masalah *"The Glass Door Problem"* di Fase 3.

---

## 3. Secure Hierarchical Deterministic Wallets with Stealth Addresses (IACR ePrint 2022/627)
* **File Jurnal:** [`jurnal/Secure-Hierarchical-Deterministic-Wallet-Stealth-Address.md`](file:///workspaces/Zeltra-Protocol/jurnal/Secure-Hierarchical-Deterministic-Wallet-Stealth-Address.md)
* **Problem di Sistem Klasik:**
  Derivasi kunci deterministik (BIP-32) jika dikombinasikan secara naif dengan stealth address dapat membocorkan *master private key* jika salah satu *ephemeral private key* anak terekspos (*hardened vs non-hardened vulnerability*).
* **Inovasi Paper:**
  Struktur pembagian level derivasi kunci asimetris: pemisahan yang terbukti aman antara *spending key* ($sk$), *nullifier key* ($nk$), dan *viewing key* ($vk$).
* **Implementasi di Zeltra SDK:**
  * Memvalidasi dan memperkuat derivasi kunci master di [`zeltra-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs) (`from_seed`) agar tahan terhadap serangan kebocoran child key.

---

## 4. PriSrv: Private Service Discovery for Anonymous Clients (IACR ePrint 2024/1783)
* **File Jurnal:** [`jurnal/PriSrv-Private-Service-Discovery.md`](file:///workspaces/Zeltra-Protocol/jurnal/PriSrv-Private-Service-Discovery.md)
* **Problem di Sistem Klasik:**
  Klien SDK yang mencari relayer dengan fee terendah atau rute tercepat sering kali membocorkan IP dan karakteristik transaksinya ke jaringan relayer melalui query discovery publik.
* **Inovasi Paper:**
  Protokol gossip terenkripsi dengan *Private Information Retrieval (PIR)* ringan, memungkinkan SDK memilih relayer optimal secara anonim.
* **Implementasi di Zeltra SDK:**
  * Modul relayer selection di SDK: Klien dapat membandingkan quote fee relayer secara private tanpa metadata profiling.

---

## 5. MinMandate: Private Task-Scoped Payment Authorization for Adaptive Agent Workflows (2026)
* **File Jurnal:** [`jurnal/MinMandate-Private-Task-Scoped-Payment.md`](file:///workspaces/Zeltra-Protocol/jurnal/MinMandate-Private-Task-Scoped-Payment.md)
* **Penulis:** Ge Gao, Haining Yu, Zhichao Liu, Dongyang Zhan, Yuanxiao Zhu, Zhongyun Hua
* **Problem di Sistem Klasik:**
  Framework agentic payment yang ada (ERC-4337 paymaster, EIP-7702 delegation) mengizinkan AI agent untuk spend atas nama user, tetapi otorisasi bersifat open-ended: agent diberi budget maksimum dan bebas menggunakannya untuk merchant atau layanan apapun. User tidak bisa membatasi "hanya boleh bayar untuk X, bukan Y" tanpa menulis smart contract custom.
* **Inovasi Paper:**
  *Scoped Credential* berbasis constraint: User menerbitkan credential yang hanya valid untuk kategori pengeluaran spesifik (misal: "hanya untuk compute API calls", "hanya untuk domain tertentu", "maksimum 3 kali per hari"). Merchant yang menerima pembayaran harus membuktikan dirinya masuk kategori yang diizinkan via ZK proof of category membership — tanpa mengungkap kategori lain yang diizinkan.
* **Implementasi di Zeltra SDK:**
  * Method baru di SDK: `authorize_agent(scope: SpendScope, max_amount: u64, expiry: u64) -> ScopedCredential`.
  * `SpendScope` adalah enum yang di-encode ke dalam BLS credential saat issuance, diverifikasi di contract via tambahan gadget circuit pada `PrivateNoteCircuit`.
  * Use case langsung: AI agent yang mengelola micro-payment untuk API calls (GPT, compute, storage) tidak bisa di-exploit untuk drain wallet ke merchant arbitrary — scope constraint di-enforce secara kriptografis on-chain, bukan hanya di level aplikasi.
  * Komponen: [`zeltra-sdk/src/wallet/agent_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/agent_wallet.rs) (baru), integrate dengan `DelegProof-EIP-7702` untuk gasless execution.

---

## 6. zkTLS Invoice Attestation for Autonomous Agents (IACR ePrint 2026/199 — Pivot dari zkAgent)
* **File Jurnal:** [`jurnal/zkAgent-Verifiable-LLM-Agent-Execution-One-Shot-Transcript-Proofs.md`](file:///workspaces/Zeltra-Protocol/jurnal/zkAgent-Verifiable-LLM-Agent-Execution-One-Shot-Transcript-Proofs.md)
* **Problem di Sistem Klasik:**
  Mengeksekusi pembuktian ZK secara penuh atas seluruh inferensi LLM (*full model execution trace*) untuk setiap transaksi mikro agen AI ($0.05) terbukti tidak layak secara komputasi (*computationally infeasible*), menghabiskan gigabyte RAM dan waktu pembuktian bermenit-menit.
* **Inovasi Paper & Pivot:**
  Pivot dari model inferensi penuh ke *zkTLS Invoice Attestation*: SDK membuktikan secara kriptografis bahwa agen AI menerima faktur penagihan yang sah dari endpoint merchant/API web (misal Stripe, AWS Billing, OpenAI API) melalui sesi TLS terautentikasi tanpa membocorkan kunci API atau data rahasia pengguna.
* **Implementasi di Zeltra SDK:**
  * Komponen: `zeltra-sdk/src/agent.rs` dan modul kepatuhan `zeltra-sdk/src/compliance/`.
  * Memungkinkan agen AI mengikat pembayaran Note UTXO secara atomik ke komitmen bukti zkTLS faktur web, mencegah *ghost billing* atau pengeluaran fiktif.

---

## 7. Coral-CFG: Structured Business Data Parsing in Client SDK (IACR ePrint 2025/1420)
* **File Jurnal:** [`jurnal/Coral-CFG-Proofs.md`](file:///workspaces/Zeltra-Protocol/jurnal/Coral-CFG-Proofs.md)
* **Problem di Sistem Klasik:**
  Mengekstrak dan memverifikasi field tertentu dari payload terstruktur bisnis (JSON, TOML, format faktur B2B) di dalam sirkuit ZK konvensional membutuhkan puluhan ribu R1CS constraints yang membuat klien browser/ponsel mengalami crash memori.
* **Inovasi Paper:**
  Parser ZK berbasis tata bahasa bebas konteks (CFG) menggunakan pohon biner Left-Child Right-Sibling (LCRS) dan *Segmented Memory (Nebula)*. Mampu memparsing dan membuktikan field nilai uang, mata uang, dan status pembayaran dalam waktu 1–3 detik CPU lokal klien.
* **Implementasi di Zeltra SDK:**
  * Komponen: `zeltra-sdk/src/compliance/cfg.rs`.
  * Mengekstrak komitmen faktur bisnis secara 100% lokal di SDK tanpa membocorkan rincian invoice ke relayer atau validator blockchain, kompatibel langsung dengan spend note ZK-UTXO.

---

## 8. Device-Binding Anonymous Credentials for Mobile Wallets (IACR ePrint 2026/965)
* **File Jurnal:** [`jurnal/Device-Binding-Anonymous-Credentials-Legacy-Phones.md`](file:///workspaces/Zeltra-Protocol/jurnal/Device-Binding-Anonymous-Credentials-Legacy-Phones.md)
* **Problem di Sistem Klasik:**
  Prinsip Zeltra menetapkan penyimpanan note ZK-UTXO 100% di sisi klien (zeroized on drop). Ancaman terbesarnya adalah malware infostealer yang menyalin note dari memori perangkat lunak. Namun, chip hardware Secure Enclave / TEE di miliaran ponsel pintar iOS & Android terkunci secara paten pada kurva ECDSA P-256 dan tidak mendukung operasi kurva pairing BLS12-381.
* **Inovasi Paper:**
  Protokol *Proof-of-Possession Bulletproofs (PoP-BP)* di atas kurva representasi T-256 yang memisahkan domain Secure Enclave P-256 dengan BLS12-381:
  - Ukuran bukti sangat ringkas: ~1.47 kB (jauh di bawah limit calldata L2).
  - Waktu pembuktian prover di ponsel modern: ~208 ms; verifikasi: 35–52 ms.
  - Kompatibel penuh dengan kredensial anonim BLS12-381 + BBS+.
* **Implementasi di Zeltra SDK:**
  * Komponen: `zeltra-sdk/src/credential.rs`.
  * Mengikat note belanja ke otorisasi Secure Enclave/biometrik fisik pengguna. Sekalipun file note dicuri dari memori perangkat lunak oleh infostealer, note tersebut secara matematis mustahil dibelanjakan tanpa kunci privat yang tersimpan di dalam hardware Secure Enclave perangkat fisik.

---

## 9. DelegProof: Formal Accountability for Agent Delegation (IACR ePrint 2026/2060)
* **File Jurnal:** [`jurnal/honorable/DelegProof-EIP-7702-Account-Delegation.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/DelegProof-EIP-7702-Account-Delegation.md)
* **Penulis:** Ruifeng Qian, Chengyu Dong, Lingyu Gao, Yuchang Zhang, Zengli Guo (September 2026)
* **Problem di Sistem Klasik:**
  Mekanisme delegasi eksekusi akun (EIP-7702 / ERC-4337) untuk AI Agent rentan terhadap 4 kelas serangan: (1) cross-chain replay serangan authorization chainId-0, (2) front-run initialization, (3) storage confusion lintas re-delegation, dan (4) ERC-1271 substitution attack yang mencapai 63% delegasi berbahaya di mainnet ($2.36M kerugian).
* **Inovasi & Mitigasi Paper:**
  Analisis simbolik formal pertama (Tamarin models) untuk delegasi EIP-7702. Membuktikan 3 mitigasi formal: (a) larangan otorisasi chainId-0 mencegah cross-chain replay, (b) account-bound initialization gate untuk init authorization, dan (c) namespaced storage slots mencegah kebingungan storage.
* **Implementasi di Zeltra SDK:**
  * Komponen: [`zeltra-sdk/src/wallet/agent_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/agent_wallet.rs) dan modul delegasi gasless.
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
* **Implementasi di Zeltra SDK & Core:**
  * Komponen: [`zeltra-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs) (stealth scanning & ephemeral key transport via ECIES) dan [`zeltra-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note_circuit.rs).
  * Use Case Zeltra: **Direct Stealth Push Payments**. Memungkinkan pengguna Zeltra mengirim dana dari private note pool langsung ke pihak luar/merchant yang hanya mempublikasikan *Stealth Meta-Address* (tanpa mengungkap address publik aslinya di Arbiscan). Penerima dapat mengklaim atau membelanjakan dana via relayer gasless Zeltra tanpa pernah memegang saldo gas ETH on-chain.

---

## 11. SBM: Prove-Phase Bounded-Memory Groth16 via Read-Write Streaming (IACR ePrint 2026/1758)
* **File Jurnal / ePrint:** IACR ePrint 2026/1758 (Oktober 2026)
* **Penulis:** Kyeongtae Lee, Jihye Kim, Hyunok Oh
* **Problem Kritis:**
  Membuktikan sirkuit ZK Groth16 (`JoinSplitCircuit` 19 public inputs, 448 quadratic range gates) di dalam browser Web Worker (WASM 32-bit di mobile / client devices, DEC-041) membutuhkan alokasi memori kerja acak $O(N)$ yang besar. Pada perangkat mobile atau tab browser dengan resource terbatas, proses ini rentan memicu Web Worker crash akibat Out-of-Memory (OOM).
* **Inovasi Paper:**
  Memperkenalkan *Read-Write Streaming Groth16 Prover* yang memangkas kebutuhan working memory dari $O(N)$ menjadi **$O(\sqrt{N})$** secara drastis melalui streaming Number Theoretic Transform (NTT), streaming sparse R1CS evaluation, dan chunked Pippenger MSM. Yang paling revolusioner: **Interface Groth16 tetap identik (3 elemen grup $A, B, C$) dan verifikasi pairing smart contract Stylus EIP-2537 100% UNCHANGED (tidak perlu diubah sama sekali)**!
* **Implementasi di Zeltra SDK:**
  * Komponen: Runner prover Web Worker browser di [`zeltra-sdk/src/zk_wasm.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/zk_wasm.rs) (DEC-041) dan [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs).
  * Memungkinkan pengguna mengeksekusi pembuktian transaksi ZK-UTXO (Mode 2 In-Pool Transfer dan Mode 3 Public Spend) langsung di smartphone atau browser tanpa takut memory spike atau tab browser force-close.

---

## 12. ZKPoSP: Zero-Knowledge Proofs of Seed Provenance for Hierarchical Deterministic Wallets (IACR ePrint 2026/1508)
* **File Jurnal / ePrint:** IACR ePrint 2026/1508 (Oktober 2026)
* **Penulis:** Vincenzo Botta, Michal Pospieszalski, Emanuele Ragnoli, Justus Ranvier
* **Problem Fundamental:**
  Pada skenario institusi/B2B dan deposit CEX (Binance/Coinbase, DEC-040), entitas korporat wajib membuktikan kepada auditor pajak dan bursa bahwa note-note yang didepositkan berasal dari master seed perusahaan yang sah (bukan mixing ilegal atau dana pihak ketiga yang tidak berizin), tanpa mengekspos master spending key atau membocorkan struktur cabang rekening internal perusahaan.
* **Inovasi Paper:**
  Merumuskan *ZKPoSP* (Zero-Knowledge Proof of Seed Provenance) yang membuktikan keabsahan silsilah derivasi Hierarchical Deterministic (BIP-32 / BIP-44 / SLIP-10) di dalam sirkuit ZK secara matematis tanpa merusak format kunci publik atau mengekspos master private key dan index path derivasi rahasia.
* **Implementasi di Zeltra SDK:**
  * Komponen: Modul audit kepatuhan [`zeltra-sdk/src/compliance/exporter.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/compliance/exporter.rs) dan verifier CEX di [`zeltra-sdk/src/compliance/verifier.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/compliance/verifier.rs).
  * Mengintegrasikan bukti silsilah akun induk (corporate provenance proof) ke dalam `SelectiveDisclosureCircuit` (DEC-040A), memberikan jaminan kepatuhan institusional tingkat tinggi yang siap diaudit CEX dan regulator.

---

## 13. Secure MSM Outsourcing Computation for Mobile Client Prover (IACR ePrint 2026/686)
* **File Jurnal / ePrint:** IACR ePrint 2026/686 (Oktober 2026)
* **Penulis:** Wujie Xiong, Arefeh Rahaei, Sangwon Shin, Xinxin Fan, Taeweon Suh, Veronika Kuchta, Francesco Sica, Weidong Shi, Lei Xu
* **Problem Kritis:**
  Multi-Scalar Multiplication (MSM) menyita 70%–80% komputasi pembuktian sirkuit Groth16. Pada perangkat seluler, smartphone, atau IoT dengan CPU terbatas, pembuktian lokal menguras baterai dan memakan waktu beberapa detik. Namun, mendelegasikan perhitungan MSM ke cloud atau relayer publik secara naif akan membocorkan saksi rahasia (*witness scalars*) dan kunci belanja pengguna.
* **Inovasi Paper:**
  Merumuskan skema *Secure MSM Outsourcing*:
  1. Pengguna dapat mendelegasikan beban operasi MSM skala besar ke server komputasi pihak ketiga (misal: relayer `zeltra-node`).
  2. Kerahasiaan skalar rahasia (*witness*) dijamin terlindungi penuh via penyamaran homomorfik (*homomorphic blinding*).
  3. Klien dapat memverifikasi kebenaran hasil evaluasi MSM dari server secara instan dan murah dengan overhead verifikasi minimal.
* **Implementasi di Zeltra SDK & Node:**
  * Komponen: [`zeltra-sdk/src/zk_wasm.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/zk_wasm.rs) (modul delegasi pembuktian terblindir) dan endpoint relayer [`zeltra-node/src/handlers/`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/).
  * Menghadirkan fitur *"Zero-Knowledge Prover Acceleration"*: Pengguna smartphone dengan spesifikasi rendah dapat mendelegasikan komputasi MSM berat ke relayer Zeltra, menghasilkan waktu pembuktian ZK instan (<150 ms) tanpa sedikit pun mengekspos kunci belanja (*sk*), nilai uang, atau privasi note ke relayer.

---

## 14. Strong Resettable NIZK: Randomness Reset & Replay Defense for Web Worker Provers (IACR ePrint 2026/1036)
* **File Jurnal / ePrint:** IACR ePrint 2026/1036 (Oktober 2026)
* **Penulis:** Behzad Abdolmaleki, Matteo Campanelli, Quang Dao, Shadman Mohammadi, Nahid Roustaeifar
* **Problem Kritis:**
  Pada eksekusi in-browser Web Worker Groth16 (`zeltra-sdk/src/zk_wasm.rs`, DEC-041) dan lingkungan virtualisasi/container, siklus hidup memori sering kali mengalami reset atau restart (misal Web Worker dimatikan dan diregenerasi saat multitasking ponsel, atau crash restore pada aplikasi mobile). Kerentanan *prover randomness reset attack* (srNIZK vulnerability) memungkinkan penyerang mengekstraksi saksi rahasia (*witness recovery attack* terhadap $sk$, $nk$, nilai saldo, dan randomness note $r, \rho$) jika generator bilangan acak prover menghasilkan nilai $r, s$ yang berulang atau dapat diprediksi saat terjadi partial restart/reset.
* **Inovasi Paper:**
  Merumuskan *Strong Resettable NIZK (srNIZK)* dan *generic compiler* yang mentransformasikan pembuktian NIZK/zkSNARK standar menjadi srNIZK tanpa mengubah ukuran proof atau sirkuit/verifier smart contract. Randomness pembuktian diturunkan secara deterministik dari Pseudorandom Function (PRF) yang mengikat master seed, hashing witness rahasia, dan session context, sehingga tahan terhadap partial key resets, VM snapshot rollback, dan glitch injection.
* **Implementasi di Zeltra SDK:**
  * Komponen: Generator randomness di [`zeltra-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs) dan [`zeltra-sdk/src/zk_wasm.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/zk_wasm.rs).
  * Memastikan proving pipeline Groth16 di client/browser menghasilkan randomness yang tahan reset (*reset-resilient synthetic blinding*), melindungi kunci $sk$ dan saksi note dari ekstraksi memori akibat interupsi atau restart browser worker.

---

## 15. Provable Anonymity Guarantees Under Viewing Key Compromise & CEX Evasion Defense (IACR ePrint 2026/872)
* **File Jurnal / ePrint:** IACR ePrint 2026/872 (Oktober 2026)
* **Penulis:** Adrian Cinal
* **Problem Kritis:**
  Banyak protokol privasi mengandalkan Incoming Viewing Key ($ivk$) untuk memenuhi syarat regulasi dan audit bursa (CEX). Namun, paper ini membuktikan celah struktural fatal pada skema berbasis viewing key (seperti Zcash, Monero, Firo):
  1. Pelaku kejahatan dapat menyerahkan $ivk$ resmi kepada auditor/bursa, namun tetap dengan sangat mudah menghindari monitoring transaksi melalui jalur derivasi unlinked note atau transfer non-standard (*financial monitoring evasion*).
  2. Sebaliknya, kompromi atau kebocoran $ivk$ pada pihak ketiga dapat memicu deanonimisasi massal lintas epoch atas seluruh transaksi masuk yang tidak terkait dengan transaksi yang diselidiki.
* **Inovasi Paper:**
  Menganalisis batas-batas matematis formal dari jaminan anonimitas ketika viewing key diserahkan atau bocor, serta mendefinisikan kriteria ketat agar pengawasan keuangan (*financial oversight*) efektif tanpa membuka kerentanan pelarian atau pelanggaran privasi berlebihan.

---

## 16. DuraShare: BIP39-Native Secret Sharing with Full Manual Fallback (IACR ePrint 2026/2300)
* **File Jurnal / ePrint:** IACR ePrint 2026/2300 (Oktober 2026)
* **Penulis:** Jeroen van de Graaf, Renato Schiavinato Lopez
* **Problem Kritis:**
  Di aplikasi dompet non-kustodian seperti Zeltra Private Note Wallet ([`zeltra-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs)), master seed 128/256-bit (deriving $sk, nk, fvk$) adalah titik kegagalan tunggal (*single point of failure*). Jika pengguna kehilangan perangkat atau meninggal dunia, aset ZK-UTXO hilang permanen. Skema Shamir Secret Sharing konvensional umumnya menghasilkan string heksadesimal acak yang bergantung pada software/vendor tertentu untuk rekonstruksi; jika vendor tutup atau format software usang setelah puluhan tahun, pihak keluarga/wali hukum (*sharekeepers*) yang awam kriptografi tidak dapat memulihkan seed.
* **Inovasi Paper:**
  Merumuskan protokol **DuraShare** yang memecah seed BIP-39 (daftar 2048 kata) ke dalam $n$ share berformat kata BIP-39 asli. Skema ini memenuhi 3 pilar:
  1. *Shamir Secret Sharing* di atas lapangan prima kecil $GF(2053)$ (bilangan prima terkecil $> 2048$), sehingga setiap pecahan share tetap berupa kata-kata BIP-39 yang valid dan mudah dicatat di atas kertas.
  2. Deteksi kesalahan 2D (*row and column checksums*) yang melindungi share dari salah ketik atau kerusakan fisik kertas.
  3. MAC produk-dalam afin (*affine inner-product MAC*) di atas $GF(2053)$ untuk mengotentikasi keaslian share dan mendeteksi pemalsuan oleh sharekeeper curang.
  4. **Full Manual Fallback:** Rekonstruksi dapat dilakukan secara manual hanya dengan selembar kertas, pensil, kalkulator standar, dan instruksi sederhana tanpa membutuhkan komputer atau software vendor apa pun.
* **Implementasi di Zeltra SDK & CLI:**
  * Komponen: Fitur social backup & recovery di [`zeltra-sdk/src/wallet/`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/) dan CLI command di [`zeltra-cli/src/commands/zk.rs`](file:///workspaces/Zeltra-Protocol/zeltra-cli/src/commands/zk.rs).
  * Memberikan opsi backup master seed berstandar warisan keluarga (*estate planning & social recovery*): pengguna dapat membagikan 3-dari-5 share kertas kepada anggota keluarga tepercaya tanpa khawatir ketergantungan software di masa depan.

---

## 17. ZK Provenance Proofs for HD Wallet Derivation & Cross-Chain CCIP Anti-Hijacking (IACR ePrint 2026/2204)
* **File Jurnal / ePrint:** IACR ePrint 2026/2204 (September 2026)
* **Penulis:** Vincenzo Botta, Michal Pospieszalski, Emanuele Ragnoli, Justus Ranvier
* **Problem Kritis:**
  Dalam arsitektur dompet non-kustodian Zeltra ([`zeltra-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs)), satu master seed menghasilkan berbagai kunci: spending key ($sk$), nullifier key ($nk$), incoming viewing key ($ivk$), alamat ZSA Bech32m (`zs1...`), dan alamat publik Ethereum/Arbitrum. Pada skenario kepatuhan CEX (`DEC-040`) atau transfer lintas-rantai via Chainlink CCIP ([`zeltra-node/src/ccip_monitor.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/ccip_monitor.rs)), pengguna kerap harus membuktikan bahwa alamat penarikan tujuan (di Arbitrum L2, Ethereum L1, atau rantai lain) berasal dari dompet atau entitas yang sama tanpa mengekspos master seed atau menghubungkan seluruh alamat publik milik mereka secara transparan (*destination substitution / cross-chain hijacking attack*).
* **Inovasi Paper:**
  Merumuskan model **Provenance Proofs (ZKPoSP)**:
  1. *Linkable Zero-Knowledge Derivation Relations:* Membuktikan bahwa dua atau lebih kunci publik / alamat (bahkan pada kurva eliptik berbeda atau rantai berbeda) diturunkan secara jujur dari *anchor* atau seed dompet tersembunyi yang sama, tanpa membuka seed tersebut.
  2. *Fleksibilitas Hak Taut:* Menyediakan 4 model keterkaitan: hanya pengguna yang dapat menautkan (*designated verifier*), penautan publik dengan tag deterministik yang bertindak sebagai nullifier, atau pembuktian selektif ke otoritas kepatuhan/bursa via enkripsi tag.
  3. Membuktikan keamanan formal terhadap pemalsuan (*non-frameability*) dan ketidaktertautan (*unlinkability*).
* **Implementasi di Zeltra SDK & CEX Compliance Engine:**
  * Komponen: Modul kepatuhan bursa di [`zeltra-sdk/src/compliance/exporter.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/compliance/exporter.rs), sirkuit selective disclosure di [`zeltra-core/src/selective_disclosure_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/selective_disclosure_circuit.rs), dan monitor penarikan CCIP di [`zeltra-node/src/ccip_monitor.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/ccip_monitor.rs).
  * Menghadirkan fitur *"Provable Destination Binding"*:
    - **Anti-Hijacking CCIP:** Saat mengeksekusi transfer lintas-rantai, pengguna dapat menyertakan ZK Provenance Proof yang membuktikan bahwa alamat penerima di rantai tujuan diturunkan dari dompet yang sama dengan pengirim, mencegah relaying atau man-in-the-middle menyuntikkan alamat penarikan penyerang.
    - **CEX Whitelist Verification:** Memungkinkan pengguna membuktikan ke bursa seperti Binance/Coinbase bahwa alamat ZSA privat dan akun bursa mereka dimiliki oleh individu yang sama untuk memenuhi aturan *Travel Rule*, tanpa membocorkan riwayat transaksi lainnya.
