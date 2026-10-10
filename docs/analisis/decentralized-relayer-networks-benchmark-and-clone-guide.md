# Riset & Benchmark Decentralized Relayer Networks Web3 (2025–2026)
## Role Model Arsitektur & Pedoman Kloning untuk Zeltra Protocol

- **Status:** APPROVED ARCHITECTURAL REFERENCE & BENCHMARK
- **Lokasi Dokumen:** `docs/analisis/decentralized-relayer-networks-benchmark-and-clone-guide.md`
- **Tanggal:** 2026-10-10
- **Tujuan:** Dokumentasi komprehensif role model jaringan relayer desentralisasi terbaik di Web3 hasil penelusuran netral (*Parallel Search 2025–2026*), sebagai pedoman desain transisi Zeltra Protocol dari relayer tunggal (testnet) menuju jaringan relayer desentralisasi multi-operator (mainnet).

---

## 1. Executive Summary: Kenapa Zeltra Butuh Desentralisasi Relayer?

Dalam protokol privasi ZK-UTXO seperti Zeltra Protocol:
* **Fungsi Relayer:** Bertindak sebagai penyedia likuiditas gas (membayar gas fee ETH di blockchain Arbitrum) atas nama pengguna, sehingga dompet pengguna tidak perlu memiliki saldo ETH publik yang dapat merusak privasi (*unlinkable gas sponsorship*).
* **Kekhawatiran Arsitektur:** Jika hanya ada 1 relayer resmi milik Zeltra (backend tunggal), sistem rentan terhadap:
  1. *Single Point of Failure (SPOF):* Jika backend Zeltra down, pengguna tidak bisa membelanjakan uang privatnya.
  2. *Censorship Vulnerability:* Operator relayer tunggal dapat menolak memproses transaksi pengguna tertentu.
  3. *Infrastructural Burden:* Seluruh beban modal likuiditas gas ditanggung oleh satu entitas.

Untuk mengatasi hal tersebut, Zeltra dirancang untuk bertransisi ke **Decentralized Multi-Operator Relayer Network**. 

Kabar baiknya: Berdasarkan matematika ZK Groth16 yang kita bangun di [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) dan [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md), **relayer pihak ketiga yang nakal mustahil bisa mencuri uang pengguna**, karena penerima pembayaran (`recipient`) dan pengembalian (`change note`) telah terkunci mati di dalam bukti zero-knowledge!

---

## 2. Komparasi 5 Role Model Relayer Desentralisasi Teratas di Web3 (Data 2025–2026)

Berikut adalah 5 proyek open-source paling sukses dan teruji di industri Web3 yang menjadi acuan utama implementasi:

```
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                        KOMPARASI JARINGAN RELAYER DESENTRALISASI TERATAS (2025-2026)                   │
├─────────────────────┬──────────────────────────┬────────────────────────────┬──────────────────────────┤
│ Proyek / Repositori │ Model Komunikasi         │ Mekanisme Keamanan         │ Karakteristik Kunci      │
├─────────────────────┼──────────────────────────┼────────────────────────────┼──────────────────────────┤
│ 1inch P2P Network   │ P2P Encrypted Proxy      │ Micropayment Channels      │ Enkripsi end-to-end,     │
│ (1inch/p2p-network) │ (Encrypted RPC)          │ & Zero-Knowledge Payload   │ Relayer ga bisa intip tx │
├─────────────────────┼──────────────────────────┼────────────────────────────┼──────────────────────────┤
│ Hyperbridge Relayer │ Permissionless Gossip    │ ZK Interoperability Proofs │ Zero Staking,            │
│ (polytope-labs)     │ Submissions              │ on-chain verification      │ Zero Whitelist, Bebas    │
├─────────────────────┼──────────────────────────┼────────────────────────────┼──────────────────────────┤
│ Pimlico Alto /      │ ERC-4337 Bundler         │ Mempool Guard &            │ Eksekusi anti-nyangkut,  │
│ ZeroDev Ultra-Relay │ Standard JSON-RPC        │ Auto-Replacement Gas Bump  │ manajemen lonjakan gas   │
├─────────────────────┼──────────────────────────┼────────────────────────────┼──────────────────────────┤
│ Railgun Broadcaster │ Waku P2P Gossip Network  │ zk-SNARK + Private Proof   │ Tanpa server HTTP,       │
│ (Railgun-Community) │ (Peer-to-Peer Libp2p)    │ of Innocence (PPoI)        │ 100% P2P broadcast       │
├─────────────────────┼──────────────────────────┼────────────────────────────┼──────────────────────────┤
│ Tornado Cash        │ On-Chain Relayer         │ TORN Collateral Staking &  │ Discovery terdaftar di   │
│ Relayer Registry    │ Smart Contract Registry  │ ENS Subdomain Mapping      │ smart contract on-chain  │
└─────────────────────┴──────────────────────────┴────────────────────────────┴──────────────────────────┘
```

---

## 3. Rincian Teknis & Panduan Kloning Repositori

### A. 1inch P2P Network (`1inch/p2p-network`)
* **URL GitHub:** [https://github.com/1inch/p2p-network](https://github.com/1inch/p2p-network)
* **Kategori:** *Encrypted Private Relaying Infrastructure*
* **Arsitektur:**
  - Jaringan P2P terdesentralisasi di mana node Relayer bertindak sebagai proxy perantara antara pengguna dan resolver layanan.
  - Permintaan transaksi dikirim dalam format **terenkripsi penuh**. Relayer meneruskan muatan (*payload*) tanpa mampu mendekripsi isinya, sehingga privasi dan kerahasiaan isi transaksi terjaga 100%.
  - Relayer diberi kompensasi atas bandwidth transmisi dan penjagaan privasi melalui saluran pembayaran mikro (*micropayment channels*).
* **Aspek yang Di-ATM untuk Zeltra:**
  - Pola komunikasi RPC terenkripsi antara Client SDK (`nimbus-sdk`) dan Relayer (`nimbus-node`), sehingga relayer publik tidak mengetahui identitas IP atau metadata user.

---

### B. Hyperbridge Relayer (`polytope-labs/hyperbridge`)
* **URL GitHub:** [https://github.com/polytope-labs/hyperbridge](https://github.com/polytope-labs/hyperbridge)
* **Kategori:** *Fully Permissionless & Verifiable Relayer Network*
* **Arsitektur:**
  - Protokol interoperabilitas berbasis ZK coprocessor pertama yang sepenuhnya **permissionless**: siapa pun di dunia dapat menjalankan node relayer tanpa perlu izin, tanpa pendaftaran (*no whitelisting*), dan tanpa perlu mengunci modal jaminan (*no staking*).
  - Insentif ekonomi: Siapa pun relayer yang pertama kali berhasil memancarkan transaksi dan membuktikan validitas bukti kriptografis on-chain akan secara otomatis dan atomik menerima kompensasi fee dari smart contract.
* **Aspek yang Di-ATM untuk Zeltra:**
  - Kontrak Arbitrum Stylus Zeltra (`nimbus-contracts/src/spend.rs`) mengadopsi model ini 100%: entrypoint `spend_private_note` dan `spend_joinsplit` tidak menggunakan modifier `only_owner` atau `only_relayer`. Siapa pun yang men-submit proof valid langsung menerima pencairan `execution_fee`!

---

### C. Pimlico Alto & ZeroDev Ultra-Relay
* **URL GitHub Pimlico Alto:** [https://github.com/pimlicolabs/alto](https://github.com/pimlicolabs/alto)
* **URL GitHub ZeroDev Ultra-Relay:** [https://github.com/zerodevapp/ultra-relay](https://github.com/zerodevapp/ultra-relay)
* **Kategori:** *High-Reliability Production Transaction Bundler / Relayer*
* **Arsitektur:**
  - Ditulis dalam TypeScript berkinerja tinggi, berfokus penuh pada **Transaction Inclusion Reliability**.
  - Mengelola siklus transaksi rumit:
    1. Estimasi gas dinamis berbasis fluktuasi base fee L2.
    2. *Mempool watchdog*: jika transaksi tertahan >60 detik, otomatis melakukan transaksi pengganti (*transaction replacement*) dengan gas bump +15%.
    3. Nonce synchronization yang kebal terhadap kegagalan database atau restart mendadak.
* **Aspek yang Di-ATM untuk Zeltra:**
  - Mesin eksekusi internal `nimbus-node`: kita meniru algoritma nonce manager, exponential backoff, dan gas-bump replacement milik Alto agar transaksi di Arbitrum Sepolia/One tidak pernah nyangkut.

---

### D. Railgun Broadcaster Network (`Railgun-Community`)
* **URL GitHub Node:** [https://github.com/Railgun-Community/ppoi-safe-broadcaster-example](https://github.com/Railgun-Community/ppoi-safe-broadcaster-example)
* **URL GitHub Client:** [https://github.com/Railgun-Community/waku-broadcaster-client](https://github.com/Railgun-Community/waku-broadcaster-client)
* **Kategori:** *Decentralized P2P Gossip Broadcast for ZK Privacy Protocols*
* **Arsitektur:**
  - Menghilangkan server HTTP tunggal dengan memanfaatkan protokol peer-to-peer **Waku** (jaringan gossip libp2p tahan sensor).
  - Client membungkus bukti ZK dan quote fee, lalu menyiarkannya ke topik gossip Waku. Seluruh Broadcaster yang online akan mendengarkan topik tersebut dan bersaing mengeksekusi transaksi.
  - Dilengkapi *Private Proof of Innocence (PPoI)*: prover menyertakan bukti kriptografis bahwa dananya bukan bagian dari daftar sanksi (OFAC) tanpa membuka identitasnya.
* **Aspek yang Di-ATM untuk Zeltra:**
  - Modul client gossip untuk rilis mainnet jangka panjang (Fase 3), menyempurnakan pertahanan kepatuhan [`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md).

---

### E. Tornado Cash Relayer Registry
* **URL GitHub Relayer:** [https://github.com/tornadocash/tornado-relayer](https://github.com/tornadocash/tornado-relayer)
* **URL GitHub Registry Contract:** [https://github.com/tornadocash/relayer-registry](https://github.com/tornadocash/relayer-registry)
* **Kategori:** *On-Chain Staking & Decentralized Relayer Discovery*
* **Arsitektur:**
  - Smart contract `RelayerRegistry` mencatat daftar operator relayer publik yang telah men-stake token jaminan.
  - Frontend membaca registry on-chain untuk menampilkan opsi relayer, memeringkat relayer berdasarkan reputasi dan fee termurah, serta mendistribusikan beban transaksi secara merata.
* **Aspek yang Di-ATM untuk Zeltra:**
  - Model *Relayer Registry Contract* di Arbitrum Stylus untuk discovery publik pada Fase 2 Mainnet.

---

## 4. Peta Jalan Transisi Relayer Zeltra (3-Phase Roadmap)

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                         ZELTRA RELAYER DECENTRALIZATION ROADMAP                        │
├───────────┬────────────────────────────────────────────────────────────────────────────┤
│ FASE 1    │ Official Relayer Node (Testnet Arbitrum Sepolia)                           │
│ (Saat Ini)│ - Frontend nembak backend nimbus-node resmi Zeltra.                        │
│           │ - Fokus: Stabilitas, verifikasi kriptografi DEC-035/036, zero tech debt.   │
├───────────┼────────────────────────────────────────────────────────────────────────────┤
│ FASE 2    │ On-Chain Relayer Registry (Mainnet v1 Launch)                              │
│           │ - Deploy smart contract `ZeltraRelayerRegistry` di Stylus.                 │
│           │ - Siapa pun bisa pasang server relayer dan mendaftar ke registry.          │
│           │ - Frontend mengimplementasikan auto-routing ke relayer termurah/tercepat.  │
├───────────┼────────────────────────────────────────────────────────────────────────────┤
│ FASE 3    │ P2P Gossip Encrypted Network (Mainnet v2 Sovereign Mode)                   │
│           │ - Integrasi protokol Waku / Libp2p ke dalam nimbus-sdk.                    │
│           │ - Frontend membroadcast transaksi secara P2P tanpa tahu IP server relayer. │
│           │ - 100% sensor-resistant & zero-knowledge end-to-end.                       │
└───────────┴────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Bukti Keamanan Kriptografis: Mengapa Relayer Pihak Ketiga Aman 100%?

Bahkan ketika Zeltra membuka jaringannya untuk relayer pihak ketiga anonim di internet, protokol dijamin aman secara matematis:

1. **Uang Tidak Bisa Dicuri (*Theft Immunity*):**
   Pada sirkuit `PrivateNoteCircuit` dan `JoinSplitCircuit`, alamat penerima transfer (`recipient`) dan nilai pembayaran (`merchant_amount`) diikat ke dalam bukti Groth16. Jika relayer nakal mencoba mengganti `recipient` menjadi alamat dompetnya sendiri, verifikasi pairing EIP-2537 (`0x0f`) di kontrak Stylus **langsung revert**.
2. **Saldo Pengguna Tidak Bisa Dimata-matai (*Zero-Knowledge Surveillance Immunity*):**
   Dengan arsitektur **Paginated Bulk/Range MMR Sync** (DEC-036 Seksi 7.4), dompet pengguna mengunduh daun komitmen secara gelondongan (1.000 daun per batch = ~32 KB). Bukti inklusi pohon dihitung secara lokal di perangkat pengguna. Relayer tidak pernah tahu daun mana yang dimiliki pengguna.
3. **Relayer Kebal Terhadap Serangan Spam/Griefing (*CPU Preflight Protection*):**
   Berdasarkan [`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md), node relayer memverifikasi bukti Groth16 secara lokal pada CPU sebelum memancarkan transaksi ke blockchain, sehingga penyerang tidak bisa menguras saldo ETH relayer dengan bukti palsu.
4. **Relayer Selalu Untung (*Economic Solvency*):**
   Setiap transaksi mengganti 100% biaya gas ETH relayer dalam bentuk USDC ditambah margin keuntungan bersih **+15% markup**, menjamin partisipasi operator relayer independen selalu menguntungkan secara komersial.

---

## 6. Cheat-Sheet Perintah Kloning untuk Riset Tim

Untuk mengeksplorasi kode sumber kelima repositori role model di atas secara lokal:

```bash
# 1. 1inch P2P Encrypted Network
git clone https://github.com/1inch/p2p-network.git research/reporefer/1inch-p2p

# 2. Hyperbridge Permissionless Relayer
git clone https://github.com/polytope-labs/hyperbridge.git research/reporefer/hyperbridge

# 3. Pimlico Alto ERC-4337 Bundler
git clone https://github.com/pimlicolabs/alto.git research/reporefer/pimlico-alto

# 4. ZeroDev Ultra-Relay (Reliable Mempool Watchdog)
git clone https://github.com/zerodevapp/ultra-relay.git research/reporefer/zerodev-ultra-relay

# 5. Railgun PPoI Broadcaster Node & Client
git clone https://github.com/Railgun-Community/ppoi-safe-broadcaster-example.git research/reporefer/railgun-broadcaster
git clone https://github.com/Railgun-Community/waku-broadcaster-client.git research/reporefer/railgun-waku-client
```
