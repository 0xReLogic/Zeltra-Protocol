# DEC-040C: CEX Selective Disclosure Verifier Engine & Compliance CLI Tooling

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-040`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md))
- **Author:** Zeltra Protocol Architecture, Security & CLI Tooling Team
- **Date:** 2026-10-15
- **Applies to:**
  - `zeltra-sdk/src/compliance/verifier.rs` (Standalone lightweight verifier engine for CEX/auditors)
  - `zeltra-cli/src/commands/compliance.rs` (Implementation of compliance CLI subcommands)
  - `zeltra-cli/src/cli.rs` (Clap definitions: `ExportAudit`, `ExportCexProof`, `VerifyCexProof`, `ExportViewingKey`)
  - `zeltra-cli/src/main.rs` (Subcommand dispatch and error handling)
- **Parent & Related DECs:**
  - [`DEC-040`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md) (Master Viewing Key Export Suite Specification)
  - [`DEC-040A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040A-selective-disclosure-circuit-and-key-separation.md) (Selective Disclosure Circuit & Key Hierarchy)
  - [`DEC-040B`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040B-client-sdk-compliance-and-double-entry-audit-trail.md) (Client SDK Compliance & B2B Audit Trail)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-032-merkle-mountain-range-commitment-accumulator.md) (MMR Inclusion Verification & Peak Bagging)
  - [`DEC-034C`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034C-relayer-zero-oracle-reimbursement-and-sdk-multi-asset-wallet.md) (Relayer Routing & Multi-Asset CLI Tooling)
- **Architectural Paradigm:** *"Stateless Verifier Autonomy, Strict Verifier Binding, Cryptographic Proof of Innocence"*

---

## 1. Executive Summary & Masalah Ketergantungan Bursa Terpusat

Ketika pengguna menyetorkan aset dari sistem privasi on-chain ke bursa terpusat (seperti Binance, Coinbase, Kraken, atau OKX), tim kepatuhan (*Anti-Money Laundering / KYC*) bursa sering kali menahan dana tersebut untuk memeriksa apakah aset berasal dari eksploitasi, pencurian, atau alamat yang terkena sanksi OFAC ([`DEC-026`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)).

### Kebutuhan Staf Kepatuhan Bursa:
1. **Stateless Verification:** Staf bursa tidak boleh dipaksa menginstal dompet Zeltra, tidak boleh memegang rahasia privat apa pun, dan hanya membutuhkan alat verifikasi ringan yang dapat memvalidasi berkas JSON secara independen menggunakan RPC Arbitrum publik.
2. **Anti-Impersonation & Anti-Replay:** Penyerang yang mencuri berkas bukti milik orang lain tidak boleh dapat menggunakan berkas tersebut untuk mengklaim deposit atas akunnya sendiri di bursa.
3. **Pembuktian Integritas On-Chain:** Verifikator harus dapat memastikan bahwa note yang bersangkutan benar-benar pernah dicatat di Merkle Mountain Range (MMR) on-chain dan bahwa transaksi pencairan dana memang benar mentransfer dana ke alamat deposit bursa pengguna.

**Solusi Penetapan DEC-040C:**
1. Menyediakan **Standalone Verifier Engine** di `zeltra-sdk::compliance::verifier` yang mengeksekusi 5 tahap validasi matematis tanpa *side effects*.
2. Memperluas `zeltra-cli` dengan perintah intuitif untuk menghasilkan dan memverifikasi bukti kepatuhan.

---

## 2. Struktur Data Paket Bukti CEX (`zeltra_cex_proof_v1.json`)

```rust
// zeltra-sdk/src/compliance/models.rs
use serde::{Deserialize, Serialize};

/// Payload bukti kepatuhan selektif satu note untuk verifikasi bursa terpusat.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CexProofPayload {
    pub schema_version: String, // "zeltra.compliance.cex.v1"
    pub generated_at: u64,
    pub verifier_binding: VerifierBindingPayload,
    pub asset: AssetDisclosurePayload,
    pub note_cryptography: NoteCryptographyPayload,
    pub mmr_proof: MmrProofPayload,
    pub ownership_zk_proof: OwnershipZkProofPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifierBindingPayload {
    pub verifier_id: String,     // Misal: "BINANCE_KYC_DESK"
    pub account_uid: String,     // Misal: "USER-98124712"
    pub nonce: String,           // Heksadesimal 16-byte nonce unik
    pub expires_at: u64,         // Unix timestamp batas waktu berlaku (maks 24 jam)
    pub challenge_hash: String,  // Blake2s-256 digest dalam hex
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetDisclosurePayload {
    pub asset_id: u8,            // 0: WETH, 1: USDC
    pub symbol: String,          // "WETH" atau "USDC"
    pub circuit_unit_name: String, // "GWEI" atau "MICRO_USDC"
    pub circuit_decimals: u8,    // 9 (WETH) atau 6 (USDC)
    pub erc20_decimals: u8,      // 18 (WETH) atau 6 (USDC)
    pub raw_circuit_value: String,
    pub value_packed_scalar: String,
    pub human_value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteCryptographyPayload {
    pub commitment: String,
    pub owner_key: String,
    pub rho: String,
    pub randomness: String,
    pub nullifier: String,
    pub nullifier_status: String, // "SPENT" atau "UNSPENT"
    pub spent_tx_hash: String,
    pub payout_recipient: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MmrProofPayload {
    pub leaf_index: u64,
    pub leaf_count: u64,
    pub peak_index: usize,
    pub sibling_path: Vec<String>,
    pub mmr_peaks: Vec<String>,
    pub mmr_root: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnershipZkProofPayload {
    pub protocol: String, // "GROTH16_BLS12_381_DISCLOSURE_V1"
    pub public_inputs: Vec<String>,
    pub proof_bytes: String,
}
```

---

## 3. Algoritma Mesin Verifikator Mandiri (5-Stage Verification Engine)

Modul `zeltra_sdk::compliance::verifier` menjalankan verifikasi deterministik secara berurutan:

```rust
// zeltra-sdk/src/compliance/verifier.rs
use crate::compliance::models::*;
use crate::error::ComplianceError;

pub struct CexProofVerifier;

impl CexProofVerifier {
    /// Menjalankan verifikasi 5 tahap secara fail-closed terhadap berkas CexProofPayload.
    pub async fn verify<C: BlockchainRpcClient>(
        payload: &CexProofPayload,
        expected_verifier_id: &str,
        current_time: u64,
        rpc_client: &C,
    ) -> Result<VerificationSummary, ComplianceError> {
        // --- TAHAP 1: Validasi Format & Bit-Packing Re-komputasi Komitmen ---
        let asset_id = payload.asset.asset_id;
        if asset_id > 1 {
            return Err(ComplianceError::InvalidAssetId(asset_id));
        }
        let raw_val: u64 = payload.asset.raw_circuit_value.parse()
            .map_err(|_| ComplianceError::InvalidNumericFormat)?;
        
        // value_packed = value + asset_id * 2^64 (DEC-034A)
        let value_packed = crate::note::compute_value_packed(raw_val, asset_id);
        let owner_key = parse_fr(&payload.note_cryptography.owner_key)?;
        let rho = parse_fr(&payload.note_cryptography.rho)?;
        let rand = parse_fr(&payload.note_cryptography.randomness)?;
        
        let expected_cm = parse_fr(&payload.note_cryptography.commitment)?;
        let computed_cm = crate::note::note_commitment_packed(value_packed, owner_key, rho, rand);
        if computed_cm != expected_cm {
            return Err(ComplianceError::CommitmentPreimageMismatch);
        }

        // --- TAHAP 2: Verifikasi Jalur Merkle Mountain Range (MMR, DEC-032) ---
        let siblings: Vec<[u8; 32]> = payload.mmr_proof.sibling_path.iter()
            .map(|s| parse_bytes32(s)).collect::<Result<_, _>>()?;
        let peaks: Vec<[u8; 32]> = payload.mmr_proof.mmr_peaks.iter()
            .map(|s| parse_bytes32(s)).collect::<Result<_, _>>()?;
        let claimed_root = parse_bytes32(&payload.mmr_proof.mmr_root)?;

        let computed_root = crate::merkle::verify_mmr_inclusion_and_bag(
            payload.mmr_proof.leaf_index,
            expected_cm,
            &siblings,
            payload.mmr_proof.peak_index,
            &peaks,
        )?;
        if computed_root != claimed_root {
            return Err(ComplianceError::InvalidMmrInclusionProof);
        }

        // Verifikasi apakah mmr_root terdaftar pada smart contract Stylus on-chain
        let is_root_valid = rpc_client.is_accepted_note_root(claimed_root).await?;
        if !is_root_valid {
            return Err(ComplianceError::MmrRootNotAcceptedOnChain);
        }

        // --- TAHAP 3: Verifikasi Verifier Binding & Kedaluwarsa ---
        if payload.verifier_binding.verifier_id != expected_verifier_id {
            return Err(ComplianceError::VerifierIdMismatch {
                expected: expected_verifier_id.to_string(),
                actual: payload.verifier_binding.verifier_id.clone(),
            });
        }
        if current_time > payload.verifier_binding.expires_at {
            return Err(ComplianceError::ProofExpired);
        }

        // Hitung ulang challenge hash
        let expected_challenge = crate::compliance::compute_verifier_challenge(
            &payload.verifier_binding.verifier_id,
            &payload.verifier_binding.account_uid,
            payload.generated_at,
            payload.verifier_binding.expires_at,
            &payload.verifier_binding.nonce,
            &payload.note_cryptography.commitment,
            &payload.note_cryptography.nullifier,
            &payload.note_cryptography.spent_tx_hash,
            &payload.note_cryptography.payout_recipient,
        );
        if expected_challenge != payload.verifier_binding.challenge_hash {
            return Err(ComplianceError::ChallengeHashMismatch);
        }

        // --- TAHAP 4: Verifikasi Bukti Groth16 NIZK (SelectiveDisclosureCircuit) ---
        let proof_bytes = hex::decode(&payload.ownership_zk_proof.proof_bytes)
            .map_err(|_| ComplianceError::InvalidProofBytes)?;
        let (ch_hi, ch_lo) = crate::evm::split_256_to_128_limbs(&expected_challenge);
        
        let nullifier_fr = parse_fr(&payload.note_cryptography.nullifier)?;
        let public_inputs = vec![owner_key, rho, nullifier_fr, ch_hi, ch_lo];
        
        let is_zk_valid = crate::selective_disclosure_circuit::verify_disclosure_proof(
            &proof_bytes,
            &public_inputs,
        )?;
        if !is_zk_valid {
            return Err(ComplianceError::InvalidDisclosureZkProof);
        }

        // --- TAHAP 5: Verifikasi Status On-Chain Transaksi Pencairan ---
        let nf_bytes = parse_bytes32(&payload.note_cryptography.nullifier)?;
        let is_spent = rpc_client.is_nullifier_spent(nf_bytes).await?;
        if !is_spent {
            return Err(ComplianceError::NullifierNotSpentOnChain);
        }

        // Verifikasi bahwa tx receipt mentransfer aset ke payout_recipient
        rpc_client.verify_spend_receipt_recipient(
            &payload.note_cryptography.spent_tx_hash,
            asset_id,
            &payload.note_cryptography.payout_recipient,
        ).await?;

        Ok(VerificationSummary {
            status: "SUCCESS".into(),
            asset_symbol: payload.asset.symbol.clone(),
            amount_human: payload.asset.human_value.clone(),
            recipient: payload.note_cryptography.payout_recipient.clone(),
            account_uid: payload.verifier_binding.account_uid.clone(),
            verified_at: current_time,
        })
    }
}
```

---

## 4. Spesifikasi Perintah CLI (`zeltra-cli`)

Perintah-perintah baru diintegrasikan ke dalam `zeltra-cli`:

### A. Parser Clap (`zeltra-cli/src/cli.rs`)
```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    // ... Subcommands yang sudah ada ...

    /// Mengekspor laporan audit pembukuan kas privat (Format JSON atau CSV)
    ExportAudit {
        #[arg(short = 'f', long, default_value = "json")]
        format: String, // "json" atau "csv"
        #[arg(short = 'A', long, default_value = "all")]
        asset: String,  // "all", "usdc", atau "weth"
        #[arg(long)]
        from: Option<String>, // Format: YYYY-MM-DD
        #[arg(long)]
        to: Option<String>,   // Format: YYYY-MM-DD
        #[arg(short = 'o', long)]
        out: String,          // Path output berkas
    },

    /// Mengekspor bukti kepatuhan satu note untuk diserahkan ke CEX
    ExportCexProof {
        #[arg(short = 'c', long)]
        commitment: String,
        #[arg(short = 'v', long)]
        verifier: String,      // Misal: "BINANCE_KYC_DESK"
        #[arg(short = 'u', long)]
        account_uid: String,   // Misal: "USER-98124712"
        #[arg(long)]
        spent_tx: String,      // Transaksi pengeluaran dana ke CEX
        #[arg(short = 'r', long)]
        recipient: String,     // Alamat deposit pengguna di bursa
        #[arg(short = 'o', long)]
        out: String,
    },

    /// Memvalidasi berkas bukti CEX (Dapat dijalankan oleh staf bursa/auditor)
    VerifyCexProof {
        #[arg(short = 'f', long)]
        file: String,
        #[arg(short = 'v', long)]
        expected_verifier: String,
        #[arg(long)]
        rpc: Option<String>,
    },

    /// Mengekspor kunci baca terpisah untuk pendelegasian audit internal
    ExportViewingKey {
        #[arg(short = 't', long, default_value = "fvk")]
        key_type: String, // "ivk", "ovk", atau "fvk"
        #[arg(short = 'o', long)]
        out: String,
    },
}
```

---

## 5. Matriks Pengujian Formal (`zeltra-cli` & `zeltra-sdk`)

1. `test_cex_verifier_full_flow_success`: Menghasilkan bukti yang valid untuk deposit USDC ke Binance dan memastikan verifikator mengembalikan `SUCCESS`.
2. `test_cex_verifier_rejects_wrong_verifier_id`: Berkas bukti yang ditujukan untuk `"BINANCE_KYC_DESK"` ditolak jika diverifikasi dengan parameter `"COINBASE_COMPLIANCE"`.
3. `test_cex_verifier_rejects_expired_proof`: Berkas bukti dengan `current_time > expires_at` ditolak dengan error `ProofExpired`.
4. `test_cex_verifier_rejects_invalid_zk_proof`: Memodifikasi signature atau public input `challenge_hi` menghasilkan penolakan instan `InvalidDisclosureZkProof`.
5. `test_cex_verifier_rejects_forged_mmr_tree`: Komitmen yang tidak terdaftar pada pohon MMR on-chain gagal pada verifikasi root `MmrRootNotAcceptedOnChain`.
6. `test_cli_export_audit_creates_valid_csv`: CLI command `export-audit --format csv` menghasilkan file CSV dengan header akuntansi lengkap dan sel terproteksi dari injeksi.
