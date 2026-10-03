# Nimbus CLI — 02: Commands Reference

Rujukan lengkap 8 perintah CLI yang didefinisikan pada [`nimbus-cli/src/cli.rs`](file:///workspaces/Zeltra-Protocol/nimbus-cli/src/cli.rs) beserta flag, tipe data, dan outputnya.

---

## 1. Kategori Kunci & Threshold

### `generate-keys`
Membuat keypair issuer baru (kunci rahasia $sk_{iss}$ dan kunci publik $pk_{iss}$).

* **Sintaks:**
  ```bash
  nimbus-cli generate-keys
  ```
* **Output:**
  * `Secret Key (sk_iss)` (Hex)
  * `Public Key (pk_iss)` (Hex)

---

### `split-key`
Membagi kunci rahasia issuer menjadi $n$ pecahan share dengan ambang batas (*threshold*) $t$ menggunakan algoritma Shamir Secret Sharing.

* **Sintaks:**
  ```bash
  nimbus-cli split-key --sk <SECRET_KEY_HEX> [-t <THRESHOLD>] [-n <TOTAL>]
  ```
* **Argumen:**
  * `-s, --sk <HEX>`: Kunci rahasia issuer dalam format Hex. *(Wajib)*
  * `-t, --threshold <USIZE>`: Jumlah minimum share untuk merekonstruksi signature (Default: `3`).
  * `-n, --total <USIZE>`: Total jumlah share yang dihasilkan (Default: `5`).
* **Output:**
  * Daftar share privat per index: `Share i (Index i)` (Hex)
  * Daftar share publik per index: `Public Share i` (Hex)

---

### `aggregate`
Menggabungkan partial signatures dari sejumlah threshold guardian yang cukup menggunakan interpolasi Lagrange menjadi satu masked signature utuh.

* **Sintaks:**
  ```bash
  nimbus-cli aggregate --indices <INDICES> --signatures <SIGNATURES>
  ```
* **Argumen:**
  * `-i, --indices <STRING>`: Daftar indeks guardian dipisahkan koma, misal: `"1,2,3"`. *(Wajib)*
  * `-s, --signatures <STRING>`: Daftar partial signature dalam Hex dipisahkan koma. *(Wajib)*
* **Output:**
  * `Masked Signature (sigma_tilde)` (Hex)

---

## 2. Kategori Protokol BDHKE (Blind Signatures)

### `blind`
Client membutakan pesan (misal: ephemeral public key atau nullifier pre-image) dengan blinding factor acak $r$.

* **Sintaks:**
  ```bash
  nimbus-cli blind --message <MESSAGE_STRING>
  ```
* **Argumen:**
  * `-m, --message <STRING>`: String pesan yang akan dibutakan. *(Wajib)*
* **Output:**
  * `Blinded Message (X)`: Titik pada kurva $G_1$ (Hex).
  * `Blinding Factor (r)`: Skalar rahasia pembuta pada $\mathbb{Z}_p$ (Hex).

---

### `sign`
Issuer atau leader menandatangani pesan yang sudah dibutakan dengan kunci rahasianya dan kunci masking sekali pakai ($k$).

* **Sintaks:**
  ```bash
  nimbus-cli sign --blinded <BLINDED_HEX> --sk <SECRET_KEY_HEX>
  ```
* **Argumen:**
  * `-b, --blinded <HEX>`: Blinded message $X$ dari client. *(Wajib)*
  * `-s, --sk <HEX>`: Secret key issuer (atau guardian share). *(Wajib)*
* **Output:**
  * `Masked Signature (sigma_tilde)`: $\tilde{\sigma} \in G_1$ (Hex).
  * `Masking Key (k)`: Kunci masking sekali pakai $k \in \mathbb{Z}_p$ (Hex).
  * `Commitment (com_k)`: Komitmen publik $k \cdot pk_{iss} \in G_2$ (Hex).

---

### `verify-masked`
Client memverifikasi secara off-chain bahwa masked signature dari issuer valid terhadap komitmen $com_k$ sebelum menyetorkan deposit on-chain.

* **Sintaks:**
  ```bash
  nimbus-cli verify-masked --blinded <BLINDED_HEX> --com-k <COM_K_HEX> --masked-sig <MASKED_SIG_HEX>
  ```
* **Pengecekan Matematis:**
  $$e(\tilde{\sigma}, G_2) == e(X, com_k)$$
* **Output:** `VALID` atau `INVALID`.

---

### `unmask`
Client membuka masking setelah kunci $k$ dirilis on-chain untuk menghasilkan tanda tangan final $\alpha$.

* **Sintaks:**
  ```bash
  nimbus-cli unmask --masked-sig <MASKED_SIG_HEX> -r <R_HEX> -k <K_HEX>
  ```
* **Rumus Operasi:**
  $$\alpha = (r \cdot k)^{-1} \cdot \tilde{\sigma} \in G_1$$
* **Output:**
  * `Unmasked Signature (alpha)`: Tanda tangan BLS bersih pada $G_1$ (Hex).

---

### `verify`
Verifier publik atau merchant memverifikasi tanda tangan final unmasked terhadap pesan asli dan kunci publik issuer.

* **Sintaks:**
  ```bash
  nimbus-cli verify --message <MESSAGE_STRING> --sig <SIG_HEX> --pk <PK_HEX>
  ```
* **Pengecekan Matematis:**
  $$e(\alpha, G_2) == e(H(m), pk_{iss})$$
* **Output:** `VALID` atau `INVALID`.
