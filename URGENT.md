# URGENT: Voucher System UX Problem

**Date:** 2026-06-14  
**Severity:** CRITICAL  
**Status:** BLOCKING MAINNET

---

## TL;DR

Nimbus pakai **voucher model** (1 deposit = 1 credential = 1 spend). Protocol lain (Railgun, Aztec) pakai **deposit/balance model** (1 deposit = persistent balance, bisa spend berulang).

**Voucher system bikin UX mati** karena:
1. User harus deposit **exact amount** tiap transaksi
2. Gas fee + protocol fee = user harus hitung manual
3. Kalau deposit kurang → spend fail
4. Kalau deposit lebih → sisa locked (uang hilang)
5. Protocol lain ga punya masalah ini karena **user deposit sekali, balance persist**

---

## Problem 1: Gas Fee Uncertainty

### Voucher System (Nimbus Sekarang)

```
User mau bayar merchant $50

Langkah:
1. Cek gas price sekarang → 20 gwei
2. Hitung execution fee → $0.50
3. Hitung protocol fee (0.25%) → $0.125
4. Total cost = $50 + $0.50 + $0.125 = $50.625
5. User deposit $50.625
6. Bikin credential untuk $50.625
7. Spend $50.625 ke merchant

MASALAH:
- Kalau gas spike (20 → 50 gwei) → execution fee naik ke $1.25
- Total cost sekarang = $50 + $1.25 + $0.125 = $51.375
- Tapi credential cuma $50.625 → SPEND FAIL (insufficient funds)
- User harus deposit ulang, bikin credential baru
```

### Deposit System (Railgun/Aztec)

```
User mau bayar merchant $50

Langkah:
1. User deposit $100 USDC (sekali)
2. Masuk ke private balance: $100
3. Spend $50 ke merchant
   - Protocol auto-deduct: $50 + fee ($0.125) + gas ($0.50) = $50.625
   - Balance sekarang: $49.375
4. Mau spend lagi? Tinggal spend dari balance, ga perlu deposit lagi

KEUNTUNGAN:
- Gas fee dipotong dari balance, bukan dari credential
- User ga perlu hitung exact amount
- Balance persist, bisa dipakai berulang
- Kalau gas spike? Balance masih cukup ($49.375 > $0)
```

**Perbedaan kunci:**
- **Voucher:** Gas + fee harus diprediksi SEBELUM deposit
- **Deposit:** Gas + fee dipotong dari balance SESUDAH spend

---

## Problem 2: Locked Funds (Sisa Uang Hilang)

### Voucher System (Nimbus Sekarang)

```
User mau bayar $50, tapi gas uncertain

User deposit $55 (buffer biar aman)
→ Bikin credential $55
→ Actual cost = $50 + $0.50 (gas) + $0.125 (fee) = $50.625
→ Spend $50.625 sukses
→ Sisa $4.375 LOCKED DI CONTRACT

Kenapa locked?
- Credential = 1 nullifier = 1 spend
- Nullifier udah dipakai (one-time use)
- Ga ada "withdraw remainder" function
- Sisa $4.375 = DEAD FUNDS (ga bisa diambil lagi)

Impact:
- 10 transaksi × $55 deposit × $4.375 locked = $43.75 HILANG
- User kehilangan 8% dari deposit
- UX nightmare: "kok uang gw ilang?"
```

### Deposit System (Railgun/Aztec)

```
User mau bayar $50

User deposit $55
→ Balance: $55
→ Spend $50.625
→ Balance: $4.375

Sisa $4.375 MASIH BISA DIPAKAI:
- Transfer ke user lain
- Swap ke token lain
- Withdraw ke public wallet
- Atau biarin aja, nanti dipakai

Impact:
- $0 HILANG
- User punya kontrol penuh atas balance
- UX kayak bank: "saldo gw masih ada"
```

**Perbedaan kunci:**
- **Voucher:** Sisa = dead funds (locked forever)
- **Deposit:** Sisa = masih bisa dipakai (persistent balance)

---

## Problem 3: Repeated Deposits (UX Ribet)

### Voucher System (Nimbus Sekarang)

```
User mau transaksi 5x sehari:
- Pagi: bayar kopi $5
- Siang: transfer temen $20
- Sore: bayar subscription $10
- Malem: kirim keluarga $50
- Besok: bayar merchant $30

Tiap transaksi butuh:
1. Hitung gas + fee (manual atau SDK)
2. Deposit exact amount
3. Tunggu deposit confirm
4. Bikin credential
5. Tunggu credential ready
6. Spend
7. Done

Total: 5× (deposit + credential + spend) = 15 tx on-chain
User experience: CAPEK, LAMA, RIBET
```

### Deposit System (Railgun/Aztec)

```
User mau transaksi 5x sehari:
- Pagi: bayar kopi $5
- Siang: transfer temen $20
- Sore: bayar subscription $10
- Malem: kirim keluarga $50
- Besok: bayar merchant $30

Langkah:
1. Deposit $200 sekali (pagi)
2. Balance: $200
3. Spend $5 (kopi) → balance $195
4. Spend $20 (temen) → balance $175
5. Spend $10 (subscription) → balance $165
6. Spend $50 (keluarga) → balance $115
7. Besok spend $30 → balance $85

Total: 1 deposit + 5 spend = 6 tx on-chain
User experience: KAYAK WALLET BIASA
```

**Perbedaan kunci:**
- **Voucher:** N deposit untuk N transaksi
- **Deposit:** 1 deposit untuk N transaksi

---

## Problem 4: Gas Markup Danger

### Kenapa Gas Markup Bahaya di Voucher System

```
Relayer bayar gas upfront, minta reimbursement dari user

Flow voucher:
1. User deposit $50 + gas estimate ($0.50) = $50.50
2. Bikin credential $50.50
3. User sign spend $50.50
4. Relayer broadcast spend
5. Actual gas = $0.75 (karena gas spike)
6. Relayer rugi $0.25 (bayar $0.75, cuma dapet reimbursement $0.50)

ATAU:

1. User deposit $50 + gas estimate ($0.50) = $50.50
2. Bikin credential $50.50
3. Actual gas = $1.00 (gas spike parah)
4. Relayer minta user deposit ulang $50 + $1.00 = $51.00
5. User: "Lah kok beda? Udah gw deposit $50.50 tadi"
6. UX: CONFUSING, UNPREDICTABLE

ATAU LEBIH PARAH:

1. User deposit $50 + gas estimate ($0.50) = $50.50
2. Bikin credential $50.50
3. Relayer markup gas 50% → charge user $0.75
4. Actual gas = $0.50
5. Relayer untung $0.25 (overcharge)
6. User: "Kok mahal? Gw cek gas price cuma $0.50"
7. UX: FEELS LIKE SCAM
```

### Deposit System (Railgun/Aztec)

```
Flow deposit:
1. User deposit $100 ke balance
2. User spend $50 ke merchant
3. Contract auto-deduct:
   - $50 (merchant)
   - $0.125 (protocol fee 0.25%)
   - $0.50 (actual gas, real-time)
4. Total deducted: $50.625
5. Balance: $49.375

Keuntungan:
- Gas dipotong real-time dari balance
- Ga ada "estimate vs actual" mismatch
- User lihat exact deduction
- Relayer ga perlu markup (ga ada risk)
- UX: TRANSPARENT, PREDICTABLE
```

**Perbedaan kunci:**
- **Voucher:** Gas estimate SEBELUM deposit → mismatch risk
- **Deposit:** Gas deducted SAAT spend → exact, no risk

---

## Problem 5: Protocol Fee Calculation

### Voucher System (Nimbus Sekarang)

```
Protocol fee = 0.25% dari amount

User mau spend $50:
- Protocol fee = $50 × 0.0025 = $0.125
- User harus deposit $50.125

Tapi tunggu, gas juga kena fee?
- Gas = $0.50
- Protocol fee dari gas? = $0.50 × 0.0025 = $0.00125
- Total fee = $0.125 + $0.00125 = $0.12625

Atau fee cuma dari merchant amount?
- Protocol fee = $0.125 (dari $50)
- Gas = $0.50 (no fee)
- Total = $50 + $0.125 + $0.50 = $50.625

CONFUSING! User ga tau fee dihitung dari mana.
```

### Deposit System (Railgun/Aztec)

```
Protocol fee = 0.25% dari spend amount

User spend $50:
- Protocol fee = $50 × 0.0025 = $0.125
- Gas = $0.50 (dipotong terpisah)
- Total deducted = $50 + $0.125 + $0.50 = $50.625
- Balance before: $100
- Balance after: $49.375

CLEAR! User lihat breakdown:
- Spent: $50
- Fee: $0.125
- Gas: $0.50
- Total: $50.625
```

**Perbedaan kunci:**
- **Voucher:** Fee calculation ambiguous (dari mana?)
- **Deposit:** Fee calculation clear (dari spend amount)

---

## Problem 6: Why Protocols Use Deposit System

### Railgun/Aztec Design

```
1. User deposit USDC/ETH ke contract (sekali)
2. Contract mint "encrypted notes" atau "private balance"
3. User bisa:
   - Transfer ke user lain (private)
   - Swap token (private)
   - Interact DeFi (private)
   - Withdraw ke public (unshield)
4. Balance persist, bisa dipakai berulang
5. Privacy maintained via ZK proofs
```

**Kenapa ini works:**
- Balance = sum of all unspent notes
- Tiap spend = consume old notes + create new notes
- Privacy intact (notes encrypted, ZK proof)
- User experience = kayak wallet biasa

### Nimbus Sekarang

```
1. User deposit USDC ke contract (tiap transaksi)
2. Contract bikin 1 credential (BLS signature)
3. User bisa:
   - Spend credential (1x only)
   - Done
4. Credential mati (nullifier dipakai)
5. Mau pakai lagi? Deposit ulang
```

**Kenapa ini sucks:**
- Credential = 1-time use voucher
- Ga ada "balance" concept
- User harus deposit berulang
- UX = voucher, bukan wallet

---

## Problem 7: Real-World Impact

### User Story: Alice mau pakai Nimbus

```
Alice: "Gw mau kirim $50 ke temen gw secara private"

Langkah 1: Cek gas price
- Gas sekarang: 25 gwei
- Execution fee estimate: $0.60

Langkah 2: Hitung total cost
- Merchant amount: $50
- Protocol fee (0.25%): $0.125
- Execution fee: $0.60
- Total: $50.725

Langkah 3: Deposit
- Alice deposit $50.725 USDC
- Tunggu confirm (30 detik)

Langkah 4: Bikin credential
- Contract bikin BLS credential untuk $50.725
- Tunggu signing (5 detik)

Langkah 5: Spend
- Alice spend $50.725 ke temen
- Actual gas: $0.80 (gas spike!)
- SPEND FAIL: insufficient funds ($50.725 < $50.925)

Langkah 6: Alice frustrasi
- "Lah kok fail? Udah gw deposit $50.725"
- Harus deposit ulang $50.925
- Credential lama LOCKED ($50.725 hilang)
- Total spent: $50.725 (locked) + $50.925 (baru) = $101.65
- Temen cuma dapet $50
- Alice kehilangan $51.65

Alice: "Gw ga mau pakai Nimbus lagi"
```

### User Story: Alice pakai Railgun

```
Alice: "Gw mau kirim $50 ke temen gw secara private"

Langkah 1: Deposit (sekali)
- Alice deposit $100 USDC
- Tunggu confirm (30 detik)
- Balance: $100

Langkah 2: Transfer
- Alice transfer $50 ke temen
- Protocol auto-deduct:
  - $50 (merchant)
  - $0.125 (fee)
  - $0.80 (actual gas)
  - Total: $50.925
- Balance: $49.075

Langkah 3: Done
- Temen dapet $50
- Alice masih punya $49.075 di balance
- Bisa dipakai lagi kapan aja

Alice: "Enak banget, kayak pakai wallet biasa"
```

**Perbedaan:**
- **Nimbus:** Alice kehilangan $51.65, frustrasi, uninstall
- **Railgun:** Alice masih punya balance, happy, recommend ke temen

---

## Root Cause Analysis

### Kenapa Nimbus Pakai Voucher System?

**Kemungkinan 1: Simpler Implementation**
- 1 deposit = 1 credential = 1 spend
- Ga perlu track balance
- Ga perlu UTXO management
- Easier to reason about

**Kemungkinan 2: Privacy Concerns**
- Nullifier = one-time use = privacy guarantee
- Kalau bisa spend 2x → privacy leak?
- Tapi Railgun/Aztec solve ini dengan ZK + encrypted notes

**Kemungkinan 3: Use Case Misunderstanding**
- Lu pikir Nimbus = payment rail (bayar sekali, selesai)
- Bukan private wallet (balance persist, transaksi berulang)
- Tapi reality: user butuh wallet, bukan voucher

**Kemungkinan 4: Belum Study Protocol Lain**
- Lu fokus di BLS signature + ZK circuit
- Ga riset Railgun/Aztec balance model
- Jadi design dari scratch, tanpa reference

---

## Why This is CRITICAL

### 1. User Retention
- User coba sekali → frustrasi → uninstall
- Ga ada second chance
- Competitor (Railgun, Aztec) lebih gampang

### 2. Word of Mouth
- User bilang ke temen: "Jangan pakai Nimbus, ribet"
- Negative reviews
- Hard to recover reputation

### 3. Adoption Barrier
- Merchant ga mau integrate
- "User harus deposit exact amount? Ribet amat"
- "Ga bisa balance persist? Ga praktis"

### 4. Competitive Disadvantage
- Railgun: $2B volume, 100M TVL
- Aztec: ribuan user aktif
- Nimbus: ??? (belum launch, UX udah bermasalah)

---

## Possible Solutions

### Option 1: Migrate to UTXO Model (2-3 bulan)

**Design:**
```
Deposit $100
→ Contract bikin 10 encrypted notes (@ $10 each)
→ User spend 5 notes ($50)
→ 5 notes sisa masih di balance
→ Privacy maintained (notes encrypted + ZK proof)
```

**Pros:**
- UX kayak Railgun/Aztec
- Balance persist
- No locked funds
- Proven model

**Cons:**
- Major redesign
- 2-3 bulan development
- Need new ZK circuit
- Migration complexity

### Option 2: Account Model with ZK (2-3 bulan)

**Design:**
```
Deposit $100
→ Balance $100 (encrypted)
→ Spend $50
→ ZK proof: "I have >= $50, without revealing balance"
→ Balance $50 (encrypted)
```

**Pros:**
- Simpler than UTXO
- Balance persist
- No locked funds

**Cons:**
- Need new ZK circuit
- Less privacy (balance range proof needed)
- 2-3 bulan development

### Option 3: Hybrid - Auto-Split Vouchers (1-2 minggu)

**Design:**
```
Deposit $100
→ SDK auto-bikin 10 credentials ($10 each)
→ User spend 5 credentials ($50)
→ 5 credentials sisa masih ada
→ Bisa dipakai lagi

Atau:
→ SDK auto-bikin denominations:
  - 5× $1
  - 3× $5
  - 2× $10
  - 1× $20
  - 1× $50
→ User pilih yang pas
```

**Pros:**
- Quick fix (1-2 minggu)
- No contract change
- SDK handle complexity
- Reduce locked funds

**Cons:**
- UX masih awkward (user lihat banyak voucher)
- Still voucher-based (not true balance)
- Denomination management complex

### Option 4: Voucher + Refund Mechanism (1-2 minggu)

**Design:**
```
Deposit $55 (buffer)
→ Bikin credential $55
→ Actual spend $50.625
→ Sisa $4.375 auto-refund setelah 24h
→ Atau: user manual claim refund
```

**Pros:**
- Quick fix (1-2 minggu)
- No locked funds
- Simple implementation

**Cons:**
- User harus tunggu 24h buat refund
- Gas cost untuk refund (kalau kecil, rugi)
- Still voucher-based (not true balance)

---

## Recommendation

**Short-term (1-2 minggu):** Option 3 (Auto-Split) atau Option 4 (Refund)
- Quick fix biar bisa launch
- Reduce locked funds
- UX masih awkward tapi better than nothing

**Long-term (2-3 bulan):** Option 1 (UTXO) atau Option 2 (Account)
- Proper redesign
- UX kayak Railgun/Aztec
- Competitive advantage

---

## Questions to Answer

1. **Apa target user Nimbus sebenarnya?**
   - Merchant payment rail? (bayar sekali, selesai)
   - Private wallet? (balance persist, transaksi berulang)
   - Privacy DeFi? (swap, lend, borrow privately)

2. **Kenapa lu pilih voucher model dari awal?**
   - Simpler implementation?
   - Specific use case?
   - Atau memang belum study protocol lain?

3. **Berapa budget untuk redesign?**
   - Quick fix (1-2 minggu)?
   - Proper redesign (2-3 bulan)?
   - Atau launch dulu dengan voucher, fix later?

4. **Apa prioritas sekarang?**
   - Launch ASAP (dengan voucher)?
   - Fix UX dulu (tunda launch)?
   - Atau hybrid (launch + parallel redesign)?

---

## Next Steps

**Malam ini:**
1. Lu baca document ini
2. Lu jawab questions di atas
3. Lu decide direction

**Besok:**
1. Kalau quick fix → gw mulai implement Option 3/4
2. Kalau proper redesign → gw bikin technical spec untuk UTXO/Account model
3. Kalau launch dulu → gw fokus finish HT-10 tests, fix UX later

---

## Appendix: Protocol Comparison

| Feature | Nimbus (Now) | Railgun | Aztec |
|---------|--------------|---------|-------|
| **Model** | Voucher | UTXO (encrypted notes) | UTXO (encrypted notes) |
| **Deposit** | Per-transaction | Once, balance persist | Once, balance persist |
| **Balance** | No (1-time voucher) | Yes (sum of notes) | Yes (sum of notes) |
| **Reusable** | No | Yes | Yes |
| **Locked Funds** | Yes (remainder) | No | No |
| **Gas Handling** | Estimate before deposit | Deduct from balance | Deduct from balance |
| **UX** | Voucher (ribet) | Wallet (easy) | Wallet (easy) |
| **Privacy** | BLS signature | ZK-SNARK | ZK-SNARK |
| **Maturity** | Pre-launch | $2B volume, 100M TVL | Ribuan user aktif |

**Conclusion:** Nimbus UX jauh di belakang competitor. Perlu redesign atau accept niche market (merchant payment only, bukan general wallet).
