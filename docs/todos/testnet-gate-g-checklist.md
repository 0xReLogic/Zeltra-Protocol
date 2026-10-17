# Gate G — Testnet Hard-Test & Crash Recovery Checklist

> **Priority:** Tier 0 — Mainnet Blocker  
> **Status:** Pending (Dijalankan sekali di akhir setelah Circuit Freeze & MPC selesai)  
> **Target Network:** Arbitrum Sepolia  
> **Referensi:** [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 4) & [`research/decisions/DEC-038`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)

---

## Prinsip Gladi Bersih (Single-Pass Execution)

1. **Zero Gas & Deploy Waste:** Smart contract Stylus dan relayer node **hanya di-deploy SEKALI** menggunakan verifying keys resmi hasil upacara Trusted Setup MPC.
2. **Fail-Closed Validation:** Seluruh uji negatif wajib membuktikan bahwa jika transaksi gagal/revert, state on-chain, saldo kontrak, dan nullifiers tidak berubah sepeser pun.
3. **Multi-Asset Isolation:** Setiap pengujian dijalankan terpisah untuk rel USDC dan rel WETH.

---

## 1. Matrix Lifecycle On-Chain (Positive Scenarios)

- [ ] **Direct Deposit Inflow (0 bps Fee):**
  - Deposit 10 USDC menghasilkan private note exact 10 USDC (0 bps deposit fee).
  - Deposit 0.01 Native ETH / WETH menghasilkan private note exact 0.01 WETH (auto-wrap payable di kontrak).
- [ ] **Mode 2 In-Pool Transfer (Merchant Masking - 0 bps Fee):**
  - Alice transfer 5 USDC ke ZSA Bob (`zs1...`).
  - Output 1 terenkripsi untuk Bob, Output 2 kembalian Alice.
  - Recipient `0x0`, protocol fee `0`, nol token keluar dari smart contract.
  - Bob memindai event on-chain via `recover_notes_batch()` dan saldo bertambah tepat 5 USDC.
- [ ] **Mode 3 Public Spend with Change (45 bps Flat Fee):**
  - Spend 2 USDC ke alamat merchant publik.
  - Payout ke merchant tepat 2 USDC.
  - Protocol fee flat 45 bps (0.45%) dan execution fee tertagih sesuai signed quote.
  - Change note tercatat di MMR dan saldo dompet pengirim ter-update akurat.
- [ ] **Second Spend via Change Note:**
  - Belanja kedua sukses menggunakan note kembalian dari transaksi sebelumnya (membuktikan siklus UTXO berantai).
- [ ] **Withdraw-All (Zero Remainder):**
  - Menarik seluruh sisa saldo kas privat ke wallet publik owner.
  - User note liability di smart contract berkurang hingga tepat 0.
- [ ] **Relayer Execution Fee Claim:**
  - Relayer memanggil `claim_execution_fees` untuk aset USDC dan WETH.
  - Akumulasi fee tertransfer utuh ke relayer; klaim kedua seketika gagal/revert tanpa mutasi saldo.

---

## 2. Uji Negatif & Pertahanan Adversary (Negative Scenarios)

- [ ] **Double-Spend Rejection (Nullifier Replay):**
  - Mengirim ulang transaksi dengan nullifier yang sama harus revert (`NullifierAlreadySpent`).
  - Saldo kontrak dan status MMR tidak boleh berubah.
- [ ] **Forged Change Note Value:**
  - Memanipulasi nominal kembalian lebih besar 1 unit terkecil langsung gagal verifikasi Groth16.
- [ ] **Wrong Owner Key & Corrupted Merkle Path:**
  - Bukti dengan Merkle path palsu atau owner key salah ditolak oleh verifier EIP-2537.
- [ ] **Non-Aliasing Nullifier Guard:**
  - Request JoinSplit dengan `nullifier_1 == nullifier_2` ditolak seketika (`DuplicateNullifierInSameTx`).
- [ ] **Strict 3-Mode Violation:**
  - Mode 2 dengan `recipient != 0x0` langsung di-revert.
  - Mode 2 dengan `protocol_fee > 0` langsung di-revert.
  - Mode 3 dengan `recipient == 0x0` langsung di-revert (`ZERO_RECIPIENT_ON_PUBLIC_SPEND`).
- [ ] **Memo Length Tampering:**
  - Calldata memo dengan panjang bukan 96 byte langsung ditolak (`INVALID_MEMO_LENGTH`).
- [ ] **Concurrent Spend Race Condition:**
  - Dua transaksi spend bersamaan yang mencoba menghabiskan note yang sama: hanya satu yang berhasil, transaksi kedua revert tanpa corrupting state.

---

## 3. Crash Recovery & Idempotency Testing

- [ ] **Crash Sebelum Enqueue:**
  - Relayer mati mendadak sebelum antrean SQLite di-commit $\rightarrow$ client aman untuk retry.
- [ ] **Crash Setelah Broadcast, Sebelum Receipt:**
  - Relayer restart $\rightarrow$ mempool watchdog mendeteksi transaksi in-flight di mempool L2 dan menunggu konfirmasi tanpa mengirim transaksi ganda.
- [ ] **Chain Reorg & Replacement:**
  - Penggantian transaksi dengan gas bump +15% tidak menimbulkan duplikasi note atau desinkronisasi MMR.
- [ ] **Siloed Multi-Asset Solvency Invariant Check:**
  - Memverifikasi secara on-chain: `balance(USDC) >= total_liabilities(USDC)` dan `balance(WETH) >= total_liabilities(WETH)` setelah setiap batch transaksi.

---

## 4. Cross-Chain CCIP Testnet Flow

- [ ] **End-to-End CCIP Message Passing:**
  - Source chain broadcast $\rightarrow$ Chainlink CCIP Router $\rightarrow$ Destination Stylus execution $\rightarrow$ Destination receipt confirmation.
- [ ] **CCIP Replay & Unauthorized Sender Defense:**
  - Mengirim ulang `message_id` yang sama ditolak (`CCIP_MESSAGE_ALREADY_PROCESSED`).
  - Pengirim yang tidak masuk allowlist ditolak (`CCIP_SENDER_NOT_ALLOWED`).
