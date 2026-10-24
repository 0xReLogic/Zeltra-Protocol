# DEC-043A: Zeltra Shielded Address (ZSA), One-Way Key Derivation & Deterministic ECDH Note Randomness

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md))
- **Author:** Zeltra Protocol Architecture & Cryptography Team
- **Date:** 2026-10-16
- **Applies to:**
  - `zeltra-core/src/note.rs` (ZSA struct, key derivation hierarchy, deterministic ECDH note randomness, memo AEAD)
  - `zeltra-core/src/lib.rs` (Re-exports ZSA, deterministic memo encryption/decryption primitives)
- **Parent & Related DECs:**
  - [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md) (Master In-Pool Transfer Blueprint)
  - [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-037-note-encryption-memo-and-viewing-key-recovery.md) (48-Byte Note Plaintext & 96-Byte AEAD Memo Protocol)
  - [`DEC-040A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040A-selective-disclosure-circuit-and-key-separation.md) (Cryptographic Key Separation: sk vs nk vs fvk)
  - [`DEC-034A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md) (Canonical Field Bit-Packing $I_0 = v + \text{asset\_id} \cdot 2^{64}$)
  - [`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md) (Universal 2-in-2-out JoinSplit 19-PI Circuit)
- **Security Audit Resolusi:** AUD-1 (Permanent Fund Lock), AUD-2 (Unrecoverable Randomness), AUD-3 (Self-Change Decryption Failure), AUD-7 (Topologic Leak & Low-Order Point).
- **Architectural Paradigm:** *"Zero Spend Authority Leakage, Pure-Field Commitment Parity, Deterministic In-Band Randomness"*

---

## 1. Executive Summary & Dilema Kriptografi P2P Transfer

Dalam sistem shielded transfer antar-pihak (Alice mentransfer ke Bob di dalam pool), arsitektur Zeltra dihadapkan pada **tiga dilema kriptografi fundamental**:

### A. Dilema Reduksi Kurva Silang vs R1CS Sirkuit (AUD-1)
Jika pengirim menurunkan kunci pemilik note penerima dengan mereduksi kunci publik X25519 (`pk_ivk`) ke dalam field skalar BLS12-381:
$$owner\_key_{bob} = \text{Fr::from\_be\_bytes\_mod\_order}(\&pk\_ivk_{bob})$$
maka ketika Bob mencoba membelanjakan note tersebut di masa depan menggunakan [`JoinSplitCircuit`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs#L824), sirkuit menghitung nullifier key:
$$nk = \text{Poseidon\_W3}(in\_owner\_key, \text{DOMAIN\_NULLIFIER})$$
$$nf = \text{Poseidon\_W5}([nk, \rho, \text{epoch}, 0], \text{DOMAIN\_NULLIFIER})$$
Karena $in\_owner\_key$ harus berupa $owner\_key_{bob}$ agar komitmen daun MMR cocok, sementara nullifier sah Bob diturunkan dari $sk_{bob}$ (di mana $sk_{bob} \ne \text{Fr}(pk\_ivk_{bob})$), maka **witness pembuktian belanja Bob tidak akan pernah bisa memenuhi sirkuit Groth16**. Dana Bob terkunci permanen (*Permanent Fund Lock*)!
Sebaliknya, jika Bob memberikan $sk_{bob}$ mentah kepada Alice, hak belanja Bob bocor seketika.

### B. Dilema Ketiadaan Trapdoor Randomness pada Memo 48-Byte (AUD-2)
Struktur [`NotePlaintext`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note.rs#L483) tepat berukuran **48 byte**:
$$\text{NotePlaintext} = [v \ (8\text{B}) \parallel \rho \ (32\text{B}) \parallel \text{asset\_id} \ (8\text{B})]$$
Payload ini **sama sekali tidak menyimpan trapdoor randomness $r$**. Jika Alice mengundi $r_1 \stackrel{\$}{\leftarrow} \mathbb{F}_r$ secara acak di memori komputernya, Bob saat mendekripsi memo dari event on-chain tidak memiliki informasi apapun mengenai nilai $r_1$. Akibatnya, Bob mustahil merekonstruksi komitmen $cm_1 = \text{Poseidon}(v, owner\_key, \rho, r)$ dan tidak bisa membuktikan kepemilikan note.

### C. Dilema Kunci Enkripsi Memo Kembalian & Dummy Leak (AUD-3 & AUD-7)
1. Pengirim wajib mengenkripsi memo kembalian Output 2 menggunakan **kunci publik X25519** (`pk_ivk_alice: [u8; 32]`), bukan scalar rahasia $ivk$.
2. Pada transfer pas tanpa kembalian ($v_{out,2} = 0$), memo dilarang diisi `[0u8; 96]` karena $u = 0$ adalah low-order point Curve25519 (RFC 7748) dan membocorkan topologi transfer exact payment ke pengamat on-chain.

---

## 2. Arsitektur Zeltra Shielded Address (ZSA - 64 Bytes)

Untuk menyelesaikan AUD-1 secara tuntas tanpa recompilation sirkuit, Zeltra mendefinisikan struktur **Zeltra Shielded Address (ZSA)** berukuran tepat 64 byte:

```text
Master Seed (32 bytes)
       │
       ├──► derive_spending_key() ──► sk ∈ Fr (254.7-bit scalar)
       │         │
       │         └──► Poseidon_W3(sk, 0, DOMAIN_OWNER_KEY) ──► owner_key_fr ∈ Fr (32 bytes)
       │
       └──► derive_viewing_keys() ──► ivk (X25519 secret) ──► pk_ivk (X25519 public, 32 bytes)
                                                                 │
                                ┌────────────────────────────────┴──────────────────────────────┐
                                ▼                                                               ▼
                     ZSA Payload (64 bytes)                                           Bech32m Encoding
        [owner_key_fr (32 bytes) || pk_ivk (32 bytes)]                             zs1zeltra8x9q2m... (98 chars)
```

### A. Komponen ZSA
1. **`owner_key_fr` (`ark_bls12_381::Fr`, 32 bytes):**
   - Diturunkan secara satu arah (*one-way preimage resistant*):
     $$\text{owner\_key\_fr} = \text{Poseidon\_W3}([sk, 0], \text{DOMAIN\_OWNER\_KEY})$$
   - Berfungsi sebagai identitas pemilik note di dalam komitmen Poseidon-W5 daun MMR.
   - Karena merupakan fungsi satu arah, mempublikasikan `owner_key_fr` tidak membocorkan $sk$.
2. **`pk_ivk` (`[u8; 32]`):**
   - Kunci publik X25519 (titik Montgomery pada Curve25519).
   - Digunakan oleh pengirim untuk mengeksekusi pertukaran kunci Diffie-Hellman ephemeral ($esk$) saat mengenkripsi memo AEAD.

### B. Serialisasi Hex & Bech32m
- **Hex Representation:** 128 karakter heksadesimal (opsional dengan prefiks `0x`).
  - Byte 0..32: `owner_key_fr` (big-endian 32-byte representation).
  - Byte 32..64: `pk_ivk` (32-byte raw public key).
- **Bech32m Representation:** Human-readable string dengan HRP (Human-Readable Part) `"zs"` dan varian checksum Bech32m (BIP-350):
  ```text
  zs1zeltra... (panjang total ~98 karakter)
  ```

---

## 3. Protokol Deterministic ECDH Note Randomness

Menyelesaikan AUD-2 dengan mengikat trapdoor hiding randomness $r$ ke *shared secret* pertukaran kunci Diffie-Hellman:

```text
Pengirim (Alice)                                              Penerima (Bob)
----------------                                              --------------
1. Undi esk <--$ OsRng                                        
2. Hitung epk = X25519_Public(esk)                             
3. Hitung shared_secret S = X25519(esk, pk_ivk_bob)            
4. Undi rho_1 <--$ OsRng                                       
5. Turunkan r_1 = derive_ecdh_note_randomness(S, rho_1)        
6. Hitung cm_1 = note_commitment(v, asset_id, owner_key, rho_1, r_1)
7. Enkripsi pt(v, rho_1, asset_id) via ChaCha20Poly1305(S)     
8. Siarkan cm_1 dan 96B memo [epk || ciphertext || tag]        
                                           ══════════════════► 1. Pindai event NoteCommitmentAppended
                                                               2. Ekstrak epk dari memo (byte 0..32)
                                                               3. Hitung shared_secret S = X25519(ivk_bob, epk)
                                                               4. Dekripsi ciphertext -> dapat (v, rho_1, asset_id)
                                                               5. Turunkan r_1 = derive_ecdh_note_randomness(S, rho_1)
                                                               6. Rekonstruksi cm_1 = note_commitment(...)
                                                               7. cm_1 COCOK 100% dengan daun MMR!
```

### A. Formula Matematis Derivasi $r$
$$\text{DOMAIN\_NOTE\_RANDOMNESS} = \text{Keccak256}("zeltra.note.randomness.v1") \pmod r$$
$$S_{fr} = \text{Fr::from\_be\_bytes\_mod\_order}(\&S)$$
$$r = \text{Poseidon\_W5}([S_{fr}, \rho, 0, 0], \text{DOMAIN\_NOTE\_RANDOMNESS})$$

Sifat Matematis:
1. **Deterministik & Dapat Direkonstruksi:** Karena $S$ identik di kedua sisi (sifat dasar Diffie-Hellman: $esk \cdot pk\_ivk = ivk \cdot epk$) dan $\rho$ disimpan di dalam memo 48 byte, Bob selalu mendapatkan nilai $r$ yang persis sama.
2. **Hiding Property Terjamin:** Bagi pihak ketiga yang mengamati on-chain tanpa $ivk$, $S$ adalah computational Diffie-Hellman problem yang infeasible dipecahkan, sehingga $r$ bersifat pseudorandom sempurna dan komitmen $cm$ tetap *information-theoretically hiding*.

---

## 4. Spesifikasi Implementasi Rust (`zeltra-core/src/note.rs`)

### A. Domain Separator Kunci Pemilik Baru
```rust
/// Domain tag for one-way owner key derivation from spending key (DEC-043A).
pub fn domain_owner_key() -> Fr {
    domain_from_label("zeltra.owner.key.v1")
}

/// Derives the public owner key scalar (Fr) from a master spending key (DEC-043A).
pub fn derive_owner_key(spending_key: Fr) -> Fr {
    crate::poseidon::native_poseidon_w3(
        spending_key,
        Fr::from(0u64),
        domain_owner_key(),
    )
}
```

### B. Struktur `ZeltraShieldedAddress`
```rust
/// Zeltra Shielded Address (ZSA) representing a 64-byte recipient destination (DEC-043A).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ZeltraShieldedAddress {
    pub owner_key: Fr,
    pub incoming_viewing_public_key: [u8; 32],
}

impl ZeltraShieldedAddress {
    pub const HRP: &'static str = "zs";
    pub const BYTE_LEN: usize = 64;

    pub fn new(owner_key: Fr, incoming_viewing_public_key: [u8; 32]) -> Self {
        Self {
            owner_key,
            incoming_viewing_public_key,
        }
    }

    /// Derives ZSA deterministically from a master spending key and viewing key.
    pub fn from_keys(spending_key: Fr, incoming_viewing_public_key: [u8; 32]) -> Self {
        let owner_key = derive_owner_key(spending_key);
        Self {
            owner_key,
            incoming_viewing_public_key,
        }
    }

    /// Canonical 64-byte serialization (32B owner_key || 32B pk_ivk).
    pub fn to_bytes(&self) -> [u8; Self::BYTE_LEN] {
        let mut bytes = [0u8; Self::BYTE_LEN];
        bytes[0..32].copy_from_slice(&crate::fr_to_be_bytes(&self.owner_key));
        bytes[32..64].copy_from_slice(&self.incoming_viewing_public_key);
        bytes
    }

    /// Deserializes 64 bytes into ZSA. Rejects non-canonical scalar field elements (>= r).
    pub fn from_bytes(bytes: &[u8; Self::BYTE_LEN]) -> Option<Self> {
        let mut ok_bytes = [0u8; 32];
        ok_bytes.copy_from_slice(&bytes[0..32]);
        let owner_key = crate::fr_from_be_bytes(&ok_bytes)?;
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&bytes[32..64]);
        Some(Self {
            owner_key,
            incoming_viewing_public_key: pk,
        })
    }

    /// Encodes ZSA into canonical 128-character hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Parses ZSA from a 0x-prefixed or raw 128-character hex string.
    pub fn from_hex(hex_str: &str) -> Result<Self, KeyFormatError> {
        let clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        if clean.len() != 128 {
            return Err(KeyFormatError::InvalidLength {
                expected: 128,
                actual: clean.len(),
            });
        }
        let bytes = hex::decode(clean).map_err(|_| KeyFormatError::InvalidHexEncoding)?;
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(&arr).ok_or(KeyFormatError::NonCanonicalScalar { field: "owner_key" })
    }

    /// Encodes ZSA into Bech32m string format with "zs" HRP.
    pub fn to_bech32m(&self) -> Result<String, bech32::Error> {
        use bech32::{Bech32m, Hrp};
        let hrp = Hrp::parse(Self::HRP)?;
        let data = self.to_bytes();
        bech32::encode::<Bech32m>(hrp, &data)
    }

    /// Parses ZSA from a Bech32m string format with "zs" HRP.
    pub fn from_bech32m(s: &str) -> Result<Self, KeyFormatError> {
        use bech32::{Bech32m, Hrp};
        let (hrp, data) = bech32::decode(s).map_err(|_| KeyFormatError::InvalidHexEncoding)?;
        if hrp != Hrp::parse(Self::HRP).unwrap() {
            return Err(KeyFormatError::InvalidLength { expected: 64, actual: data.len() });
        }
        if data.len() != 64 {
            return Err(KeyFormatError::InvalidLength { expected: 64, actual: data.len() });
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&data);
        Self::from_bytes(&arr).ok_or(KeyFormatError::NonCanonicalScalar { field: "owner_key" })
    }
}
```

### C. Helper Enkripsi Memo Deterministik
```rust
/// Derives note randomness deterministically from a 32-byte ECDH shared secret and rho (DEC-043A).
pub fn derive_ecdh_note_randomness(shared_secret: &[u8; 32], rho: &Fr) -> Fr {
    let secret_fr = Fr::from_be_bytes_mod_order(shared_secret);
    derive_note_randomness(secret_fr, *rho)
}

/// Encrypts note plaintext into a 96-byte AEAD memo using ephemeral X25519 ECDH,
/// and returns the deterministic hiding randomness Fr matching the recipient's derivation (DEC-043A).
pub fn encrypt_note_memo_deterministic<R: RecipientViewingKey>(
    value: u64,
    rho: Fr,
    asset_id: u64,
    recipient: &R,
) -> Result<(Vec<u8>, Fr), KeyFormatError> {
    let recipient_pk_bytes = recipient.recipient_pubkey();
    let plaintext = NotePlaintext { value, rho, asset_id };
    let pt_bytes = plaintext.to_bytes();

    let mut eph_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut eph_bytes);
    let ephemeral_secret = x25519_dalek::StaticSecret::from(eph_bytes);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let recipient_public = x25519_dalek::PublicKey::from(recipient_pk_bytes);

    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);
    let randomness = derive_ecdh_note_randomness(shared_secret.as_bytes(), &rho);

    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret.as_bytes());
    let mut okm = [0u8; 44];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).map_err(|_| KeyFormatError::InvalidHexEncoding)?;

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = chacha20poly1305::Key::from(key_bytes);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&okm[32..44]);
    let nonce = chacha20poly1305::Nonce::from(nonce_bytes);

    let cipher = chacha20poly1305::ChaCha20Poly1305::new(&cipher_key);
    let ciphertext_and_tag = cipher
        .encrypt(&nonce, pt_bytes.as_ref())
        .map_err(|_| KeyFormatError::InvalidHexEncoding)?;

    let mut result = Vec::with_capacity(ENCRYPTED_MEMO_SIZE);
    result.extend_from_slice(ephemeral_public.as_bytes());
    result.extend_from_slice(&ciphertext_and_tag);
    Ok((result, randomness))
}

/// Decrypts a 96-byte AEAD note memo and simultaneously reconstructs the deterministic randomness (DEC-043A).
pub fn decrypt_note_memo_deterministic(
    encrypted_memo: &[u8],
    ivk: &IncomingViewingKey,
) -> Option<(NotePlaintext, Fr)> {
    if encrypted_memo.len() != ENCRYPTED_MEMO_SIZE {
        return None;
    }

    let mut epk_bytes = [0u8; 32];
    epk_bytes.copy_from_slice(&encrypted_memo[0..32]);
    let epk = x25519_dalek::PublicKey::from(epk_bytes);

    let static_secret = x25519_dalek::StaticSecret::from(ivk.secret_bytes());
    let shared_secret = static_secret.diffie_hellman(&epk);

    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret.as_bytes());
    let mut okm = [0u8; 44];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).ok()?;

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = chacha20poly1305::Key::from(key_bytes);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&okm[32..44]);
    let nonce = chacha20poly1305::Nonce::from(nonce_bytes);

    let cipher = chacha20poly1305::ChaCha20Poly1305::new(&cipher_key);
    let decrypted = cipher.decrypt(&nonce, &encrypted_memo[32..96]).ok()?;

    if decrypted.len() != NotePlaintext::SERIALIZED_SIZE {
        return None;
    }

    let mut pt_buf = [0u8; NotePlaintext::SERIALIZED_SIZE];
    pt_buf.copy_from_slice(&decrypted);
    let pt = NotePlaintext::from_bytes(&pt_buf)?;

    let randomness = derive_ecdh_note_randomness(shared_secret.as_bytes(), &pt.rho);
    Some((pt, randomness))
}
```

---

## 5. Matriks Pengujian Unit & Soundness (`zeltra-core`)

Sesuai aturan engineering Zeltra, implementasi DEC-043A wajib disertai **1 Positive Test + Minimal 2x Negative Tests**:

### Positive Tests:
1. `test_zsa_roundtrip_hex_and_bech32m`: Serialisasi dan deserialisasi ZSA melalui Hex dan Bech32m menghasilkan tuple `(owner_key_fr, pk_ivk)` yang identik.
2. `test_deterministic_ecdh_randomness_and_commitment_match`: Pengirim mengenkripsi memo dan memperoleh `rand_sender`. Penerima mendekripsi memo dan memperoleh `rand_receiver`. Nilai `rand_sender == rand_receiver` dan komitmen daun `cm_sender == cm_receiver` terbukti identik 100%.

### Negative Tests (Minimal 2x Positive = Minimal 4 Tests):
1. `test_zsa_rejects_non_canonical_owner_key`: Parsing ZSA dengan skalar `owner_key` $\ge r$ me-revert dengan `KeyFormatError::NonCanonicalScalar`.
2. `test_zsa_rejects_corrupted_bech32m_checksum`: Mengubah 1 karakter pada string Bech32m menggagalkan parsing.
3. `test_deterministic_memo_rejects_tampered_ciphertext`: Mengubah 1 bit pada ciphertext memo menyebabkan otentikasi tag Poly1305 gagal (mengembalikan `None`).
4. `test_deterministic_memo_fails_with_wrong_ivk`: Mencoba mendekripsi memo dengan IVK yang berbeda menghasilkan `None` (karena MAC tag mismatch).
5. `test_deterministic_memo_rejects_invalid_size`: Memo dengan panjang $\ne 96$ byte langsung ditolak sebelum operasi kriptografi.
