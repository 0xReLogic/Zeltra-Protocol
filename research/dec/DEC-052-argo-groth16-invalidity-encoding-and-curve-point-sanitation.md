# DEC-052: Argo Randomized Encoding for Groth16 Invalidity, Permissionless Fraud Gating, and Strict Subgroup Curve Point Defense

- **Status:** APPROVED ARCHITECTURAL SPECIFICATION
- **Tipe DEC:** FITUR BARU | OPTIMASI FRAUD PROOF | CIRCUIT HARDENING & AUDIT DEFENSE
- **Author:** Zeltra Protocol Security Architecture & Applied Cryptography Team
- **Date:** 2026-10-25
- **Target Crates & Components:**
  - `zeltra-core/src/evm.rs` (Strict EIP-2537 Curve Point Sanitization, Non-Infinity Rejection, and Subgroup Membership Membership Gates)
  - `zeltra-core/src/curve_opt.rs` (Eisenstein Endomorphism Subgroup $S$, Cube Root $\omega$, Coordinate Scaling for Fast Point Decoupling)
  - `zeltra-contracts/src/groth16_note_verifier.rs` & `groth16_joinsplit_verifier.rs` (Host-Level Pre-Pairing Validation & Invalid Point Immunity)
  - `zeltra-node/src/handlers/spend.rs` (Preflight Syntactic Sanitization, Non-Canonical Coordinate Filtering, and Fraud Ingress)
  - `zeltra-sdk/src/compliance/verifier.rs` (Optimistic Fraud Slashing, Zero-Knowledge In-Browser Verifier, and 1-Shot Preimage Derivation)
- **Parent & Related DECs:**
  - [`DEC-035B: Formal Scope Binding Gadget and Groth16 Statement Integrity`](DEC-035B-formal-scope-binding-gadget-and-groth16-statement-integrity.md)
  - [`DEC-036A: Universal 2-in-2-out JoinSplit R1CS Circuit Specification`](DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md)
  - [`DEC-046: Null-Branch Witness Attack Defense and Quadratic Encoding Soundness`](DEC-046-null-branch-witness-attack-defense-and-quadratic-encoding-soundness.md)
  - [`DEC-047: Augmented Jacobian Soundness and Witness-Locality Sweep`](DEC-047-jacobian-diagnostics-and-tangent-space-rigidity-auditing-for-underconstrained-circuits.md)
  - [`DEC-050: Fast Jacobian Point Tripling and Eisenstein Endomorphism on BLS12-381 (G1)`](DEC-050-fast-jacobian-tripling-and-eisenstein-endomorphism-bls12-381.md)
  - [`DEC-051: Nopenena Cryptographic Deniability, Small Decoy Sets, and Dummy Note Architecture`](DEC-051-nopenena-cryptographic-deniability-small-decoy-sets-dummy-notes.md)
- **Security Audit Targets:**
  - `AUD-30`: Small-Subgroup Confinement & Curve Point Malformation Attack (Invalid Curve Attack on Groth16 Verification)
  - `AUD-31`: Zero-Knowledge Privacy Violation under Non-Injective Target Group Mapping
  - `AUD-32`: Undecodable Fraud Proof in Optimistic Dispute State Machine (Collision in Small Endomorphism Set $S$)
- **Scientific Foundation & Primary Citations:**
  - Liam Eagen, Ying Tong Lai (Ideal Group, September 2026): *"Argo: Leaking a Secret from an Invalid Groth16 Proof"*, IACR Cryptology ePrint Archive, Report 2026/2165.
  - Liam Eagen, Ying Tong Lai (2025): *"Argo MAC: Homomorphic MACs over Elliptic Curve Groups"*, IACR ePrint 2025/18.
  - Robin Geelen, Liam Eagen (2026): *"Duty-Free Bits: Projective Garbling of Elliptic Curve Points"*, IACR ePrint 2026/37.
  - Jens Groth (2016): *"On the Size of Pairing-based Non-interactive Arguments"*, EUROCRYPT 2016.

---

## 1. Executive Summary & Ruang Lingkup Arsitektur

Pada sistem verifikasi SNARK berbasis pairing (seperti Groth16 di atas kurva BLS12-381 dan BN254), verifikasi relasi on-chain membutuhkan evaluasi produk pairing:
$$E(\pi) = e(A, B) \cdot e(C, H_1) \cdot e(x G_1, H_2) \cdot h_3 = 1 \in G_T$$
Pada infrastruktur berbiaya terbatas (seperti Bitcoin dalam BitVM, jembatan lintas rantai Chainlink CCIP, atau mekanisme tantangan fraud proof relayer layer-2), mengeksekusi sirkuit Boolean penuh untuk verifier Groth16 menuntut gerbang sirkuit raksasa (Yao Garbled Circuit dengan 10,4 miliar gerbang, 43 GB tabel garbling, dan waktu komputasi 6 menit), atau bergantung pada pihak terotorisasi khusus (*permissioned verifier* seperti Glock atau BABE).

Paper **Argo (IACR ePrint 2026/2165, Liam Eagen & Ying Tong Lai)** memperkenalkan paradigma komputasi baru: **Randomized Encoding of Groth16 Invalidity**. Argo memungkinkan encoder mengodekan bukti Groth16 sedemikian rupa sehingga:
1. **Pada Bukti VALID ($E(\pi) = 1$):** Decoder tidak memperoleh informasi apa pun tentang kunci rahasia $k \in S^n$ (kebocoran tepat 0 bit, terlindungi secara komputasional di bawah asumsi DDH pada grup sumber kedua $G_2$).
2. **Pada Bukti TIDAK VALID ($E(\pi) \ne 1$):** Decoder dapat mengekstraksi kunci rahasia $k = (\phi_1, \dots, \phi_n) \in S^n$ secara deterministik hanya melalui pencarian logaritma diskrit kecil di atas himpunan endomorfisme cincin Eisenstein $S$ ($|S| = 7$) dalam waktu **$< 1$ milidetik**, menghasilkan bukti kecurangan (*fraud proof*) yang ringkas (32 byte hash preimage) yang dapat diserahkan oleh siapa pun tanpa izin (*permissionless*).

**DEC-052 meratifikasi dua pilar fundamental Argo ke dalam Zeltra Protocol:**
1. **Adopsi Formal Argo Invalidity Encoding Protocol:** Menyediakan rail pembuktian kecurangan satu-kali-tebak (*1-shot fraud proof*) untuk relayer Zeltra, jembatan CCIP lintas rantai, dan penyelesaian sengketa mempool tanpa biaya sirkuit Boolean raksasa.
2. **Penegakan Gerbang Sanitasi Subgrup Kurva Ketat (Strict Subgroup & Curve Point Gate):** Membuktikan secara matematis dan empiris bahwa seluruh protokol Argo dan verifier Groth16 Zeltra **hanya aman jika dan hanya jika** titik-titik kurva $A \in G_1, B \in G_2, C \in G_1$ lolos gerbang sanitasi subgrup prima ($r P = \mathcal{O}$), non-infinity ($P \ne \mathcal{O}$), dan koordinat kanonikal ($x, y < q$). Tanpa gerbang ini, sistem rentan terhadap serangan kurva tidak valid (*invalid curve attack*) dan keruntuhan injektivitas logaritma diskrit.

---

## 2. Problem Formulation & Threat Model (AUD-30, AUD-31, AUD-32)

### A. Skenario Kegagalan Finansial Sebelum Penerapan Gerbang (Ungated State)

Jika verifikasi Groth16 di `zeltra-core` dan `zeltra-contracts` menerima titik kurva mentah tanpa validasi subgrup yang ketat:

1. **Serangan Kurva Tidak Valid & Pengurungan Subgrup Kerdil (`AUD-30`):**
   - Titik $B \in G_2$ atau $A \in G_1$ yang disuplai oleh penyerang dapat berasal dari subgrup berorde kecil $m$ yang membagi $q^k - 1$ pada kurva twist atau kurva Weierstrass yang bergeser.
   - Jika evaluasi pairing $E(\pi)$ jatuh ke dalam subgrup berorde $m \le 6$, maka pemetaan $\phi \mapsto E(\pi)^\phi$ untuk $\phi \in S$ ($|S| = 7$) mengalami **kolusi pigeonhole mutlak**:
     $$E(\pi)^{\phi_1} = E(\pi)^{\phi_2} \quad \text{untuk} \quad \phi_1 \equiv \phi_2 \pmod m$$
   - Akibatnya: Decoder pada Argo **gagal merekonstruksi kunci rahasia $k$** saat terjadi kecurangan (pelaku kecurangan lolos dari pemotongan jaminan/slashing).

2. **Kebocoran Informasi Privasi pada Bukti Valid (`AUD-31`):**
   - Jika sebuah bukti yang tampak valid disuntikkan dengan komponen subgrup kecil $m$, nilai terdekripsi $Y_i = [E(\pi)]_i \cdot p_i$ tidak bernilai $1$, melainkan membocorkan nilai $\phi_i \pmod m$.
   - Penyerang yang bertindak sebagai decoder dapat mengekstrak bit-bit kunci belanja $sk$ pengguna yang seharusnya tersembunyi sempurna di bawah Zero-Knowledge.

3. **Eksploitasi Titik di Tak-Hingga & Aliasing Nol (`AUD-32`):**
   - Jika titik $A = \mathcal{O}$ atau $B = \mathcal{O}$ diterima sebagai saksi sah:
     $$e(\mathcal{O}, B) = 1 \in G_T$$
   - Persamaan pairing kehilangan keterikatan linier terhadap saksi sirkuit, memungkinkan pembelanjaan note tanpa kepemilikan sah.

---

## 3. Invariants & Batasan Non-Negotiable

1. **Multi-Asset Solvency Invariant (DEC-034B / DEC-038B):**
   Eksekusi verifikasi atau pembuktian kecurangan tidak boleh mengubah neraca liabilitas:
   $$\text{Contract Token Balance} \ge \sum \text{Liabilities (User + Fee + Accrued)}$$
2. **Circuit Soundness & Tangent Space Rigidity (DEC-046 & DEC-047):**
   Setiap penambahan gerbang sanitasi tidak boleh mengubah constraint sirkuit utama (`JoinSplitCircuit` 34.571 gates, `PrivateNoteCircuit` 16 PIs, $r_{T_{\text{financial}}} = 0$).
3. **Strict Invariant Gerbang Subgrup (Subgroup Membership Gate):**
   Titik kurva eliptik publik $A \in G_1, B \in G_2, C \in G_1$ **wajib** memenuhi:
   - $P \ne \mathcal{O}$ (Bukan titik di tak-hingga).
   - $x, y < q$ (Koordinat medan kanonikal tanpa aliasing).
   - $y^2 = x^3 + b$ (Memenuhi persamaan kurva Weierstrass).
   - $r \cdot P = \mathcal{O}$ (Wajib berada di dalam subgrup prima berorde $r$).
4. **Zero-Knowledge Secrecy on Valid Proofs:**
   Keuntungan penyerang dalam membedakan kunci $k$ pada bukti valid dibatasi oleh:
   $$\text{Adv}_{\text{Argo}}(\mathcal{A}) \le \text{Adv}_{\text{DDH}}^{G_2} + \frac{1}{p} + |S|^{-n} \le 2^{-128}$$

---

## 4. Analisis Pilihan & Trade-Off Matrix

| Alternatif Pendekatan | Mekanisme Fraud Proof | Ukuran Data Offline / Online | Waktu Evaluasi Verifier | Ketahanan Terhadap Invalid Curve | Status Evaluasi |
|---|---|---|---|---|---|
| **Opsi A: Boolean Yao GC (BitVM3 / Delbrag)** | Sirkuit gerbang Boolean penuh untuk pairing 4-way | 43 GB offline / 56 kB on-chain | ~6 menit CPU | Rentan jika bit labels tidak diverifikasi kurva | **DITOLAK** (Overhead bandwidth & storage kolosal) |
| **Opsi B: Permissioned BABE / Glock** | Witness encryption linier dengan evaluator rahasia | ~22 MB offline / ~56 kB on-chain | ~175 ms | Membutuhkan setup terpercaya per-evaluator | **DITOLAK** (Melanggar prinsip permissionless) |
| **Opsi C: Argo Randomized Encoding + Strict Subgroup Gate (DEC-052)** | Group MAC + DDH MAC + Pairing Composition + Subgroup Gate | **21 kB offline / 9 kB online** | **< 1 ms discrete log (138 pairings)** | **100% Kebal (Matematis terbukti injektif pada $S$)** | **DIPILIH & DIRATIFIKASI** |

---

## 5. Spesifikasi Teknis & Detail Aljabar (IACR 2026/2165)

### A. Primitif Bilinear Groups & Groth16 Verifier

Didefinisikan grup bilinear Type-3:
$$(p, G_1, G_2, G_T, e, G, H, g)$$
di mana $|G_1| = |G_2| = |G_T| = p$ (bilangan prima besar), $G \in G_1, H \in G_2$, dan generator kanonikal $g = e(G, H) \in G_T$.

Kunci verifier Groth16 dengan input publik tunggal $x \in \mathbb{F}_p$ adalah $vk = (G_1, H_1, H_2, h_3)$, di mana:
$$H_1 = -\delta \in G_2, \quad H_2 = -\gamma \in G_2, \quad G_1 = L_1 \in G_1, \quad h_3 = e(\alpha, \beta)^{-1} e(L_0, \gamma)^{-1} \in G_T$$
Bukti Groth16 adalah $\pi = (A, B, C, x) \in G_1 \times G_2 \times G_1 \times \mathbb{F}_p$. Fungsi verifikasi:
$$E(\pi) = e(A, B) \cdot e(C, H_1) \cdot e(x G_1, H_2) \cdot h_3 \in G_T$$
Verifier menerima bukti $\pi$ jika dan hanya jika $E(\pi) = 1$.

---

### B. Group MAC, DDH MAC, dan Lemma 3 Pairing Composition

#### 1. Group MAC (Definisi 2)
Untuk elemen grup $P \in G$, pengali $\phi \in S$, dan masker acak seragam $K \in G$, Group MAC didefinisikan sebagai fungsi afin:
$$[P] = \phi P - K$$
Sifat homomorfik linier:
$$\sum_{j=1}^d \beta_j [P]_j = \left(\sum_{j=1}^d \beta_j \psi_j\right) P - \sum_{j=1}^d \beta_j K_j$$

#### 2. DDH MAC (Definisi 3)
Pada grup sumber kedua $G_2$, masker $K_i$ tidak dipilih independen seragam, melainkan mengikat skalar rahasia tunggal $t \gets \mathbb{F}_p$ yang dibagi ke seluruh replika $i \in [n]$:
$$U_i = r_i H \in G_2, \quad K_i = t U_i \implies [B]_i = \phi_i B - t U_i \in G_2$$
Pasangan $(U_i, [B]_i) = (r_i H, \phi_i B - r_i(t H))$ merupakan enkripsi ElGamal atas $\phi_i B$ di bawah kunci publik $P_{\text{pub}} = t H$.

#### 3. Lemma 3: Pairing Composition & Pembatalan Suku Silang (Cross-Term Cancellation)
Didefinisikan $[B]_i = \phi_i B - t U_i \in G_2$ dan $[A] = t A - s G \in G_1$. Ketika kedua encoding dipasangkan melalui pairing bilinear $e(\cdot, \cdot)$:
$$\begin{aligned}
e(A, [B]_i) \cdot e([A], U_i) &= e(A, \phi_i B - t U_i) \cdot e(t A - s G, U_i) \\
&= e(A, B)^{\phi_i} \cdot \underline{e(A, U_i)^{-t}} \cdot \underline{e(A, U_i)^t} \cdot e(G, U_i)^{-s} \\
&= e(A, B)^{\phi_i} \cdot e(G, r_i H)^{-s} \\
&= e(A, B)^{\phi_i} \cdot g^{-s r_i}
\end{aligned}$$
> **Inti Keunggulan Matematis:** Suku silang non-linier $e(A, U_i)^{\pm t}$ yang mengekspos $t$ **saling membatalkan secara identik** tanpa evaluasi sirkuit gerbang apa pun! Decoder memperoleh encoding dari $e(A, B)$ di bawah pengali $\phi_i$ dan masker offline $g^{s r_i}$.

---

### C. Himpunan Endomorfisme $S$ dan Dekomposisi Basis Kontraksi

Untuk kurva dengan $j$-invariant 0 (seperti BLS12-381 dan BN254), cincin endomorfisme mengandung akar kubik primitif dari satu $\omega \in \mathbb{F}_p$ ($\omega^3 \equiv 1 \pmod p$).  
Himpunan pengali kecil dipilih dari unit cincin integer Eisenstein $\mathbb{Z}[\omega]$ ditambah nol:
$$S = \{0, 1, -1, \omega, -\omega, \omega^2, -\omega^2\}, \quad |S| = 7$$
Untuk mencapai parameter keamanan $\lambda = 128$:
$$|S|^n \ge 2^{128} \implies 7^n \ge 2^{128} \implies n = \lceil 128 / \log_2 7 \rceil = 46 \text{ replika}$$

Skalar rahasia seragam $t \gets \mathbb{F}_p$ dikontraksikan melalui dekomposisi digit basis $\beta_j = (3 + \omega)^j$ sebanyak $d = 92$ digit:
$$t = \sum_{j=1}^d \beta_j \psi_j, \quad \psi_j \in S$$
$$[A] = \sum_{j=1}^d \beta_j [A]_j = \sum_{j=1}^d \beta_j (\psi_j A - s_j G) = t A - s G, \quad \text{di mana } s = \sum_{j=1}^d \beta_j s_j$$

---

### D. Protokol Tiga Fase Argo (Offline, Online, Decode)

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        PROTOKOL ENKODING ARGO INVALIDITY (DEC-052)                     │
└────────────────────────────────────────────────────────────────────────────────────────┘

 [FASE 1: OFFLINE] (Dilakukan sekali oleh Prover/Encoder sebelum proof dibuat)
   1. Sample skalar rahasia: t, s_j <- F_p; hitung s = sum beta_j s_j.
   2. Sample masker acak: r_i, v_i, w_i <- F_p untuk i in [n].
   3. Hitung basis dekomposisi: psi = Decompose(t) in S^d.
   4. Hitung titik publik: U_i = r_i * H in G_2.
   5. Hitung masker target group offline:
      p_i = h3^{phi_i} * g^{s * r_i} * e(G, H_1)^{v_i} * e(G_1, H_2)^{w_i} in G_T.
   6. Publikasikan material offline: sigma = (U_1..U_n, p_1..p_n) [Ukuran: ~21 kB].

                                     │
                                     ▼ (Klaim Pembuktian)
 [FASE 2: ONLINE] (Enkoding Bukti pi = (A, B, C, x))
   [A]_j = psi_j * A - s_j * G in G_1          (untuk j in [d])
   [B]_i = phi_i * B - t * U_i in G_2          (untuk i in [n] -- DDH MAC)
   [C]_i = phi_i * C - v_i * G in G_1          (untuk i in [n])
   [x]_i = phi_i * x - w_i     in F_p          (untuk i in [n])
   Publikasikan material online: tau = ([A], [B], [C], [x]) [Ukuran: ~9 kB].

                                     │
                                     ▼ (Evaluasi oleh Verifier / Challenger)
 [FASE 3: DECODE] (Eksekusi Dekoding Tanpa Izin)
   1. Evaluasi akumulator target group:
      [E(pi)]_i = e(A, [B]_i) * e([A], U_i) * e([C]_i, H_1) * e(G1, H2)^{[x]_i} in G_T.
   2. Lepas masker offline:
      Y_i = [E(pi)]_i * p_i = E(pi)^{phi_i} in G_T.
   3. BIFURKASI HASIL DEKODING:
      ├── KASUS A: BUKTI VALID (E(pi) == 1)
      │   ==> Y_i = 1^{phi_i} = 1 untuk seluruh i in [n].
      │   ==> Kebocoran Informasi: TEPAT 0 BIT. Kunci k tetap rahasia mutlak!
      └── KASUS B: BUKTI CACAT / CURANG (E(pi) != 1)
          ==> Y_i = E(pi)^{phi_i} != 1.
          ==> Logaritma diskrit kecil: cari phi' in S sedemikian sehingga E(pi)^{phi'} == Y_i.
          ==> Selesai dalam 7 lookup per replika (< 1 ms).
          ==> KUNCI KECURANGAN k = (phi_1..phi_n) TERBONGKAR UTUH!
```

---

### E. Pembuktian Matematis: Sebelum Gating (Rentan) vs Sesudah Gating (Aman)

#### 1. Pembuktian Kerentanan Sebelum Penerapan Gerbang (Ungated Vulnerability Proof)

Asumsikan titik kurva $A$ atau $B$ tidak digerbang, sehingga nilai verifikasi pairing $E(\pi)$ memiliki komponen non-sepele yang berada di dalam subgrup torsi kecil berorde $m$, di mana $m \mid (q^k - 1)$ dan $m < |S| = 7$ (misal $m = 3$):
$$\text{ord}(E(\pi)) = 3 \implies E(\pi)^3 = 1 \in G_T$$

Ketika decoder mengevaluasi nilai kandidat $Y_i = E(\pi)^\phi$ untuk seluruh $\phi \in S = \{0, 1, 2, 3, 4, 5, 6\}$:
$$\begin{aligned}
\phi = 0 &\implies E(\pi)^0 = 1 \\
\phi = 1 &\implies E(\pi)^1 \\
\phi = 2 &\implies E(\pi)^2 \\
\phi = 3 &\implies E(\pi)^3 = 1 \quad &\mathbf{(KOLUSI \ DENGAN \ \phi = 0)} \\
\phi = 4 &\implies E(\pi)^4 = E(\pi)^1 \quad &\mathbf{(KOLUSI \ DENGAN \ \phi = 1)} \\
\phi = 5 &\implies E(\pi)^5 = E(\pi)^2 \quad &\mathbf{(KOLUSI \ DENGAN \ \phi = 2)} \\
\phi = 6 &\implies E(\pi)^6 = 1 \quad &\mathbf{(KOLUSI \ DENGAN \ \phi = 0)}
\end{aligned}$$

**Konsekuensi Kegagalan Matematis Tanpa Gerbang:**
1. **Kegagalan Injektivitas:** Pemetaan $\phi \mapsto E(\pi)^\phi$ menghasilkan **4 kolusi dari 7 elemen**. Decoder tidak dapat membedakan apakah kunci rahasia replika adalah $\phi = 0, 3,$ atau $6$. Tingkat keberhasilan ekstraksi fraud proof anjlok dari 100% menjadi $3^{-46} \approx 0\%$. Penyerang yang mengajukan bukti palsu **kebal dari pemotongan jaminan (*slashing evasion*)**.
2. **Kebocoran Informasi Kunci pada Bukti Sah:** Jika bukti sah terkontaminasi komponen torsi $m = 3$, maka $Y_i = E_{\text{small}}^{\phi_i} \ne 1$, membocorkan nilai $\phi_i \pmod 3$. Penyerang memangkas entropi kunci dari $7^{46}$ menjadi $(7/3)^{46}$, memfasilitasi serangan pemalsuan bukti.

---

#### 2. Pembuktian Keamanan Sesudah Penerapan Gerbang (Gated Security Proof)

Zeltra menegakkan **Lima Gerbang Sanitasi Kanonikal** sebelum titik kurva menyentuh operasi pairing:
1. **Gate 1 (Equation):** Titik $P = (x, y)$ memenuhi kurva Weierstrass $y^2 = x^3 + 4$ (untuk $G_1$) atau $y^2 = x^3 + 4(1 + i)$ (untuk $G_2$).
2. **Gate 2 (Non-Infinity):** Titik $P \ne \mathcal{O}$ (menolak representasi nol byte pada input).
3. **Gate 3 (Subgroup):** $r \cdot P = \mathcal{O}$ diuji secara deterministik via perkalian skalar order subgrup prima $r$.
4. **Gate 4 (Canonical Coordinates):** Koordinat medan $x, y < q$ strictly di bawah modulus medan dasar.
5. **Gate 5 (DDH Mask Cancellation):** Multiplier $t$ pada $[A]$ identik dengan multiplier DDH mask pada $[B]_i$.

**Teorema Injektivitas Mutlak (Lemma 4 Argo):**
Karena seluruh input $A \in G_1$ dan $B \in G_2$ berada di dalam subgrup prima berorde $r$, maka nilai evaluasi pairing $E(\pi) \in G_T$ dijamin berada di dalam subgrup target berorde prima $r$.
Jika bukti $\pi$ tidak valid, maka $E(\pi) \ne 1$. Karena $r$ adalah bilangan prima besar ($r \approx 2^{254}$ pada BN254 dan $r \approx 2^{255}$ pada BLS12-381), elemen $E(\pi)$ membangkitkan subgrup siklik berorde prima tepat $r$.

Karena $|S| = 7$ dan $r$ adalah prima, maka $\gcd(|S|, r) = 1$.  
Untuk dua elemen sembarang $\phi_1, \phi_2 \in S$:
$$E(\pi)^{\phi_1} = E(\pi)^{\phi_2} \iff E(\pi)^{\phi_1 - \phi_2} = 1 \iff \phi_1 - \phi_2 \equiv 0 \pmod r$$
Karena seluruh elemen $S$ bernilai strictly di dalam interval aljabar dengan selisih $|\phi_1 - \phi_2| < r$, kesetaraan $\phi_1 \equiv \phi_2 \pmod r$ terpenuhi **jika dan hanya jika $\phi_1 = \phi_2$**.

$$\therefore \text{Pemetaan } \phi \mapsto E(\pi)^\phi \text{ terbukti secara matematis bersifat } \mathbf{STRICKTLY \ INJECTIVE \ (BIJECTIVE)} \text{ pada } S.$$
Kolusi bernilai **tepat 0 (Zero Collision)**, menjamin kepastian rekonstruksi kunci kecurangan sebesar **100,000%**.

---

## 6. Implementasi Kode Referensi di Zeltra Core (`evm.rs`)

Pada `zeltra-core/src/evm.rs`, fungsi deserialisasi `from_evm_g1` dan `from_evm_g2` diperbarui untuk menerapkan **Gerbang Sanitasi Anti-Argo**:

```rust
/// Helper to deserialize a G1 point from EVM Big-Endian format (128 bytes).
/// Menerapkan Gate 1 (Kurva), Gate 2 (Non-Infinity), Gate 3 (Subgrup), Gate 4 (Kanonikal).
pub fn from_evm_g1_sanitized(bytes: &[u8; 128]) -> Option<G1Affine> {
    use ark_ec::AffineRepr;
    use ark_serialize::CanonicalDeserialize;

    // GATE 2: Tolak titik di tak-hingga (all-zero bytes)
    if bytes.iter().all(|&b| b == 0) {
        return None; // Fail-closed! Titik infinity dilarang dalam bukti Groth16 publik
    }

    // GATE 4: Koordinat 16-byte padding terdepan wajib tepat nol
    if bytes[0..16].iter().any(|&b| b != 0) || bytes[64..80].iter().any(|&b| b != 0) {
        return None;
    }

    let mut buf = [0u8; 96];
    buf[0..48].copy_from_slice(&bytes[16..64]);
    buf[48..96].copy_from_slice(&bytes[80..128]);

    // GATE 1 & GATE 3: Arkworks deserialize_uncompressed memvalidasi persamaan kurva
    // dan keanggotaan subgrup prima secara ketat (is_in_correct_subgroup_assuming_on_curve)
    let point = G1Affine::deserialize_uncompressed(&buf[..]).ok()?;
    if !point.is_on_curve() || !point.is_in_correct_subgroup_assuming_on_curve() {
        return None;
    }

    Some(point)
}
```

---

## 7. Rencana Verifikasi Empiris & Test Gates

Sesuai standar protokol Zeltra, implementasi DEC-052 diverifikasi melalui 4 gerbang uji dengan rasio pengujian negatif minimal $2\times$:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        MATRIKS VERIFIKASI EMPIRIS DEC-052                              │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ GATE 1: Known-Answer Tests (KAT) & Happy Path                                          │
│   - Verifikasi bukti sah E(pi) == 1 menghasilkan Y_i == 1 (0 bit kebocoran).           │
│   - Verifikasi bukti cacat E(pi) != 1 membongkar kunci k in S^n 100% presisi.           │
│                                                                                        │
│ GATE 2: Negative & Boundary Adversarial Tests (Rasio >= 2x)                           │
│   - NEG-1: Titik tak-hingga (all-zeros) ditolak fail-closed (revert).                  │
│   - NEG-2: Titik subgrup kerdil torsi (ord 3) tertolak pada Gate 3.                   │
│   - NEG-3: Titik kurva invalid (y^2 != x^3 + b) tertolak pada Gate 1.                  │
│   - NEG-4: Koordinat non-kanonikal (overflow >= q) tertolak pada Gate 4.               │
│   - NEG-5: Desinkronisasi masker DDH (t != t') menyebabkan Y_i != E^phi (tertolak).    │
│   - NEG-6: Tampered input publik x_tampered != x_proof mengekstraksi k seketika.      │
│                                                                                        │
│ GATE 3: Benchmark Performa Ekstraksi Fraud                                            │
│   - Waktu rekonstruksi kunci k (n=46 replika) via table lookup: < 0,85 milidetik.      │
│   - Overhead verifikasi gerbang sanitasi pada deserializer: < 45 mikrodetik per titik. │
│                                                                                        │
│ GATE 4: E2E Integration Lifecycle                                                     │
│   - Harness Python `scripts/test_dec052_argo_verification.py` lulus 100%.              │
│   - Suite unit test `zeltra-core` dan `zeltra-contracts` lolos 100% tanpa regresi.     │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Referensi & Sitasi Akademik

1. **Liam Eagen, Ying Tong Lai (Ideal Group, 2026):** *"Argo: Leaking a Secret from an Invalid Groth16 Proof"*, IACR Cryptology ePrint Archive, Report 2026/2165.
2. **Liam Eagen, Ying Tong Lai (2025):** *"Argo MAC: Homomorphic MACs over Elliptic Curve Groups"*, IACR Cryptology ePrint Archive, Report 2025/18.
3. **Robin Geelen, Liam Eagen (2026):** *"Duty-Free Bits: Projective Garbling of Elliptic Curve Points"*, IACR Cryptology ePrint Archive, Report 2026/37.
4. **Jens Groth (2016):** *"On the Size of Pairing-based Non-interactive Arguments"*, Advances in Cryptology – EUROCRYPT 2016, LNCS vol. 9666, pp. 305–326.
5. **Dan Boneh, Xavier Boyen, Eu-Jin Goh (2005):** *"Hierarchical Identity Based Encryption with Constant Size Ciphertext"*, EUROCRYPT 2005, LNCS vol. 3494, pp. 440–456.
6. **Sergio Demian Lerner (2026):** *"FlatMerkle: In-Contract Arbitrum Accumulator & Gas Optimization"*, IACR ePrint 2026/2468.
