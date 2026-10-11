# Roadmap Prioritas Rekayasa Zeltra Protocol (2026)

Dokumen ini adalah kompas urutan pengerjaan tim. **Aturan utama:** Selesaikan fondasi inti (*Core, Contract Stylus, & Relayer Node*) terlebih dahulu sebelum melangkah ke integrasi SDK dan produk komersial.

---

## Fase 1: Fondasi Inti & Eliminasi Zero-Legacy (FOKUS UTAMA SAAT INI)
Ref: [`research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)

1. **Smart Contract Stylus (`nimbus-contracts`):**
   - Hapus seluruh peninggalan BLS/BDHKE (11 storage slot `session_*`, `_reveal_mask_key`, `verify_bls_spend`, dll).
   - Implementasikan `deposit_direct_note(amount, note_commitment, encrypted_memo)` (1 tx L2, 250ms, gas ~54k).
   - Uji verifikasi solvensi mutlak (`assets >= liabilities`).
2. **Kriptografi Inti (`nimbus-core`):**
   - Hapus fisik file `src/blind_sign.rs` dan `src/threshold.rs`.
   - Bekukan modul Poseidon Grain-128, MMR Bagging, dan sirkuit Groth16 (`PrivateNoteCircuit` & `JoinSplitCircuit`).
3. **Relayer Node (`nimbus-node`):**
   - Hapus klaster Guardian, registri kunci KMS di `kms.rs`, dan worker `deposit_indexer.rs`.
   - Bersihkan tabel SQLite (`sessions`, `stalled_sessions`, `guardian_shares`).
   - Sederhanakan relayer menjadi **Single Stateless Sequencer** (hanya melayani MMR proof sync, signed quote, dan broadcast ZK spend).

---

## Fase 2: Client SDK & Browser Prover (`nimbus-sdk`)

*Setelah Fase 1 selesai dan lulus testnet, fokus beralih ke SDK:*

1. **Integrasi Direct Inflow:**
   - Tambahkan `prepare_direct_deposit()` pada `PrivateNoteWallet` (pembuatan komitmen Poseidon 74µs di perangkat pengguna).
   - Pendaftaran instan status `UNSPENT` begitu blok Arbitrum terkonfirmasi.
2. **Viewing Key Recovery Engine (DEC-037):**
   - Pemulihan histori note dari receipt log `NoteCommitmentAppended` via ChaCha20-Poly1305.
3. **Optimasi Prover Web Worker:**
   - Bungkus eksekusi Groth16 client-side dalam Web Worker non-blocking dengan animasi checkout Apple Pay (1–2 detik).

---

## Fase 3: Tiga Pilar Produk Komersial & Adopsi Pasar
Ref: [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md#12-tiga-pilar-komersial-zeltra-2026-produk-unggulan-pasca-core-fix)

1. **Shielded Cross-Chain Checkout Widget ("Apple Pay Button" untuk Merchant):**
   - Komponen frontend React/JS 1-baris: `<ZeltraCheckout />`.
   - Merchant di Base/Polygon/Solana menerima USDC publik bersih via Chainlink CCIP.
   - Pembeli otomatis menerima **zk-Receipt** (bukti transaksi/pajak selektif via Viewing Key).
2. **HTTP 402 Shielded Agentic Micro-Pay (x402 Protocol) dengan Anti-Dust Spam:**
   - **Prinsip Anti-Spam:** Dilarang transaksi sub-sen ($0.002) langsung on-chain (mencegah pemborosan gas dan nullifier bloat).
   - **Solusi:** AI Agent menggunakan **EIP-7702 Ephemeral Session Keys** dengan anggaran harian terdelegasi, membuka **Aggregated Session Tab (Prepaid Note Pool)** untuk streaming panggilan API, dan penyedia API melakukan settlement on-chain saat mencapai batas ambang minimum (misal tiap $2.00).
3. **Sovereign Shielded RWA Staking Gateway (Opt-In Lindung Nilai Kas):**
   - Zeltra murni membuka rel/gateway (bukan alokasi otomatis). Dana kas reguler tetap 100% full-reserve USDC terisolasi.
   - B2B/Whale secara sukarela (*opt-in*) memilih sendiri instrumen RWA yang diinginkan (Ondo USDY, BlackRock BUIDL, atau Aave v3); biaya gas ditanggung oleh user (bukan subsidi protokol), namun tetap ditalangi di muka oleh Relayer (gasless ETH bagi user) dan dipotong dari saldo USDC note + 15% markup relayer.
   - Posisi kepemilikan dan bunga terakumulasi secara privat di dalam komitmen ZK Note tanpa membocorkan neraca kas ke kompetitor.
