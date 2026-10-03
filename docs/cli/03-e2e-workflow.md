# Nimbus CLI — 03: End-to-End Workflow

Panduan eksekusi simulasi kriptografi lengkap dari awal sampai akhir menggunakan `./target/debug/nimbus-cli`.

---

## 1. Alur Interaksi Kriptografi

```text
[ Client ]                             [ Issuer / Cluster ]
    |                                            |
    | 1. Generate ephemeral key & blind          |
    |    blind --message "my_key_123"            |
    |    --> (X, r)                              |
    |                                            |
    |--------- Kirim Blinded Message X --------->|
    |                                            |
    |                                            | 2. Sign blinded message
    |                                            |    sign --blinded X --sk <sk>
    |                                            |    --> (sigma_tilde, k, com_k)
    |                                            |
    |<-- Kembalikan sigma_tilde & com_k ---------|
    |    (k ditahan sampai deposit confirm)      |
    |                                            |
    | 3. Verifikasi Masked Signature             |
    |    verify-masked --blinded X ...           |
    |    --> VALID                               |
    |                                            |
    |========== Deposit On-Chain USDC ===========|
    |                                            |
    |<------- Kunci Masking k dirilis -----------|
    |                                            |
    | 4. Client Unmask Signature                 |
    |    unmask --masked-sig ... -r ... -k ...   |
    |    --> Final Signature alpha               |
    |                                            |
[ Verifier / Merchant / Stylus Smart Contract ]  |
    |                                            |
    | 5. Verifikasi Final Signature              |
    |    verify --message "my_key_123"           |
    |    --> VALID                               |
```

---

## 2. Langkah Simulasi Terminal (Step-by-Step)

### Step 1: Pembuatan Kunci Issuer
```bash
./target/debug/nimbus-cli generate-keys
```
*Catat `Secret Key` ($sk_{iss}$) dan `Public Key` ($pk_{iss}$).*

---

### Step 2: Blinding Pesan (Client)
Pesan dapat berupa address, hash intent, atau ephemeral public key:
```bash
./target/debug/nimbus-cli blind --message "ephemeral_wallet_intent_001"
```
*Catat output:*
* `Blinded Message (X)`
* `Blinding Factor (r)`

---

### Step 3: Masked Signing (Issuer)
Issuer menandatangani pesan tanpa dapat melihat isi pesan asli di dalamnya:
```bash
./target/debug/nimbus-cli sign \
  --blinded <BLINDED_MESSAGE_HEX> \
  --sk <SECRET_KEY_HEX>
```
*Catat output:*
* `Masked Signature (sigma_tilde)`
* `Commitment (com_k)`
* `Masking Key (k)` *(pada flow nyata, $k$ disimpan oleh node dan hanya dirilis setelah transaksi deposit on-chain sukses).*

---

### Step 4: Verifikasi Masked Signature Off-Chain (Client)
Sebelum menyetor uang ke contract, client memastikan issuer tidak menipu:
```bash
./target/debug/nimbus-cli verify-masked \
  --blinded <BLINDED_MESSAGE_HEX> \
  --com-k <COMMITMENT_HEX> \
  --masked-sig <MASKED_SIG_HEX>
```
*Output yang diharapkan:*
```text
VALID: Masked signature matches blinded message and commitment!
```

---

### Step 5: Unmasking (Client)
Setelah deposit USDC terkonfirmasi di smart contract dan $k$ dirilis:
```bash
./target/debug/nimbus-cli unmask \
  --masked-sig <MASKED_SIG_HEX> \
  -r <BLINDING_FACTOR_HEX> \
  -k <MASKING_KEY_HEX>
```
*Output yang didapat:*
* `Unmasked Signature (alpha)` $\rightarrow$ Ini adalah BLS credential bersih yang siap digunakan untuk belanja (*spend*).

---

### Step 6: Verifikasi Tanda Tangan Bersih (Merchant / Verifier)
Pihak penerima atau smart contract memverifikasi keaslian signature terhadap pesan asli dan public key issuer:
```bash
./target/debug/nimbus-cli verify \
  --message "ephemeral_wallet_intent_001" \
  --sig <UNMASKED_SIG_HEX> \
  --pk <PUBLIC_KEY_HEX>
```
*Output yang diharapkan:*
```text
VALID: The signature is verified and authentic under the Issuer's Public Key!
```

---

## 3. Simulasi Threshold Signing (Alternatif Step 3)

Jika menggunakan cluster 3-of-5 guardian:
1. Split secret key issuer menjadi 5 shares:
   ```bash
   ./target/debug/nimbus-cli split-key --sk <SECRET_KEY_HEX> -t 3 -n 5
   ```
2. Masing-masing guardian (misal guardian 1, 2, 3) menandatangani `Blinded Message`:
   ```bash
   ./target/debug/nimbus-cli sign --blinded <BLINDED_HEX> --sk <SHARE_1_HEX>
   ./target/debug/nimbus-cli sign --blinded <BLINDED_HEX> --sk <SHARE_2_HEX>
   ./target/debug/nimbus-cli sign --blinded <BLINDED_HEX> --sk <SHARE_3_HEX>
   ```
3. Leader menggabungkan partial signatures:
   ```bash
   ./target/debug/nimbus-cli aggregate \
     --indices "1,2,3" \
     --signatures "<SIG_1_HEX>,<SIG_2_HEX>,<SIG_3_HEX>"
   ```
4. Output yang dihasilkan identik dengan single-issuer `sigma_tilde` dan dapat dilanjutkan ke **Step 4**.
