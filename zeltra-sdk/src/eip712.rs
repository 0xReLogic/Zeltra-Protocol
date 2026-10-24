//! EIP-712 Typed Data Signing for Execution Quotes
//!
//! Implements EIP-712 signing for execution fee quotes to bind users to maximum fees
//! before transaction execution. This prevents relayer overcharging and provides
//! replay protection.

use alloy_primitives::{keccak256, Address, B256};
use alloy_sol_types::{eip712_domain, sol, Eip712Domain, SolStruct};
use serde::{Deserialize, Serialize};

sol! {
    /// Execution quote that users sign to authorize maximum execution fees
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct ExecutionQuote {
        uint8 assetId;
        address targetContract;
        uint64 baseGasFee;
        uint64 relayerMarkupFee;
        uint64 totalExecutionFee;
        uint64 expiry;
        uint256 chainId;
    }
}

/// EIP-712 domain for Zeltra Protocol
pub fn zeltra_domain(chain_id: u64, contract_address: Address) -> Eip712Domain {
    eip712_domain! {
        name: "Zeltra Protocol",
        version: "1",
        chain_id: chain_id,
        verifying_contract: contract_address,
    }
}

/// Compute the EIP-712 signing hash for an execution quote
pub fn compute_quote_hash(
    quote: &ExecutionQuote,
    chain_id: u64,
    contract_address: Address,
) -> B256 {
    let domain = zeltra_domain(chain_id, contract_address);
    quote.eip712_signing_hash(&domain)
}

/// Compute both the domain separator and struct hash for client verification
pub fn compute_quote_hashes(
    quote: &ExecutionQuote,
    chain_id: u64,
    contract_address: Address,
) -> (B256, B256) {
    let domain = zeltra_domain(chain_id, contract_address);
    let domain_separator = domain.separator();
    let struct_hash = quote.eip712_hash_struct();
    (domain_separator, struct_hash)
}

/// Sign an execution quote with a private key
///
/// # Arguments
/// * `quote` - The execution quote to sign
/// * `chain_id` - The chain ID for EIP-712 domain separator
/// * `contract_address` - The verifying contract address
/// * `private_key` - The private key to sign with (32 bytes)
///
/// # Returns
/// The signature as (v, r, s) tuple where v is the recovery id
pub fn sign_quote(
    quote: &ExecutionQuote,
    chain_id: u64,
    contract_address: Address,
    private_key: &[u8; 32],
) -> Result<(u8, [u8; 32], [u8; 32]), String> {
    use k256::ecdsa::SigningKey;

    let signing_key = SigningKey::from_bytes(private_key.into())
        .map_err(|e| format!("Invalid private key: {}", e))?;

    let hash = compute_quote_hash(quote, chain_id, contract_address);

    let (signature, recovery_id) = signing_key.sign_prehash_recoverable(&hash.0);

    let sig_bytes = signature.to_bytes();
    let r = sig_bytes[..32].try_into().unwrap();
    let s = sig_bytes[32..64].try_into().unwrap();
    let v = recovery_id.to_byte() + 27;

    Ok((v, r, s))
}

/// Verify an EIP-712 signature for an execution quote
///
/// # Arguments
/// * `quote` - The execution quote that was signed
/// * `chain_id` - The chain ID for EIP-712 domain separator
/// * `contract_address` - The verifying contract address
/// * `signature` - The signature as (v, r, s) tuple
/// * `expected_signer` - The expected signer address
///
/// # Returns
/// true if the signature is valid and from the expected signer
pub fn verify_quote_signature(
    quote: &ExecutionQuote,
    chain_id: u64,
    contract_address: Address,
    signature: (u8, [u8; 32], [u8; 32]),
    expected_signer: Address,
) -> Result<bool, String> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

    let hash = compute_quote_hash(quote, chain_id, contract_address);

    let (v, r, s) = signature;

    // Validate v value (27 or 28 for Ethereum)
    if v != 27 && v != 28 {
        return Err(format!("Invalid v value: {}", v));
    }

    let recovery_id = RecoveryId::from_byte(v - 27).ok_or("Invalid recovery id")?;

    let mut sig_bytes = [0u8; 64];
    sig_bytes[..32].copy_from_slice(&r);
    sig_bytes[32..].copy_from_slice(&s);

    let signature = Signature::from_bytes((&sig_bytes).into())
        .map_err(|e| format!("Invalid signature: {}", e))?;

    let recovered_key = VerifyingKey::recover_from_prehash(&hash.0, &signature, recovery_id)
        .map_err(|e| format!("Recovery failed: {}", e))?;

    let recovered_address = public_key_to_address(&recovered_key);

    // Verify recovered address matches expected signer
    // If recovery succeeded and address matches, the signature is valid
    Ok(recovered_address == expected_signer)
}

/// Convert a public key to an Ethereum address
fn public_key_to_address(public_key: &k256::ecdsa::VerifyingKey) -> Address {
    let public_key_bytes = public_key.to_sec1_point(false);
    let public_key_bytes = public_key_bytes.as_bytes();

    // Skip the first byte (0x04 prefix for uncompressed public keys)
    let hash = keccak256(&public_key_bytes[1..]);

    // Take the last 20 bytes of the hash
    Address::from_slice(&hash[12..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, U256};

    #[test]
    fn test_eip712_hash_deterministic() {
        let contract_address = address!("0xabcdef1234567890abcdef1234567890abcdef12");
        let chain_id = 421614u64; // Arbitrum Sepolia
        let quote = ExecutionQuote {
            assetId: 1,
            targetContract: contract_address,
            baseGasFee: 20_000,
            relayerMarkupFee: 3_000,
            totalExecutionFee: 23_000,
            expiry: 1718234567,
            chainId: U256::from(chain_id),
        };

        let hash1 = compute_quote_hash(&quote, chain_id, contract_address);
        let hash2 = compute_quote_hash(&quote, chain_id, contract_address);

        assert_eq!(hash1, hash2, "Same input should produce same hash");
    }

    #[test]
    fn test_eip712_different_chain_id() {
        let contract_address = address!("0xabcdef1234567890abcdef1234567890abcdef12");
        let quote = ExecutionQuote {
            assetId: 1,
            targetContract: contract_address,
            baseGasFee: 20_000,
            relayerMarkupFee: 3_000,
            totalExecutionFee: 23_000,
            expiry: 1718234567,
            chainId: U256::from(421614u64),
        };

        let hash1 = compute_quote_hash(&quote, 421614u64, contract_address);
        let hash2 = compute_quote_hash(&quote, 1u64, contract_address);

        assert_ne!(
            hash1, hash2,
            "Different chain IDs should produce different hashes"
        );
    }

    #[test]
    fn test_eip712_signature_roundtrip() {
        use k256::ecdsa::SigningKey;
        use rand::RngCore;

        // Generate a random private key for testing
        let mut private_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut private_key);
        // Ensure non-zero
        private_key[0] |= 1;

        // Derive the public key and address
        let signing_key = SigningKey::from_bytes((&private_key).into()).unwrap();
        let verifying_key = signing_key.verifying_key();
        let signer_address = public_key_to_address(verifying_key);

        let chain_id = 421614u64;
        let contract_address = address!("0xfedcba0987654321fedcba0987654321fedcba09");

        let quote = ExecutionQuote {
            assetId: 0, // WETH
            targetContract: contract_address,
            baseGasFee: 15_000,
            relayerMarkupFee: 2_250,
            totalExecutionFee: 17_250,
            expiry: 1718300000,
            chainId: U256::from(chain_id),
        };

        // Sign the quote
        let signature = sign_quote(&quote, chain_id, contract_address, &private_key).unwrap();

        // Verify the signature
        let is_valid = verify_quote_signature(
            &quote,
            chain_id,
            contract_address,
            signature,
            signer_address,
        )
        .unwrap();

        assert!(is_valid, "Signature should be valid");
    }

    #[test]
    fn test_eip712_wrong_signer_fails() {
        use k256::ecdsa::SigningKey;
        use rand::RngCore;

        // Generate two different keys
        let mut private_key1 = [0u8; 32];
        let mut private_key2 = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut private_key1);
        rand::thread_rng().fill_bytes(&mut private_key2);
        private_key1[0] |= 1;
        private_key2[0] |= 2;

        let signing_key2 = SigningKey::from_bytes((&private_key2).into()).unwrap();
        let wrong_signer = public_key_to_address(signing_key2.verifying_key());

        let chain_id = 421614u64;
        let contract_address = address!("0x2222222222222222222222222222222222222222");

        let quote = ExecutionQuote {
            assetId: 1,
            targetContract: contract_address,
            baseGasFee: 10_000,
            relayerMarkupFee: 1_500,
            totalExecutionFee: 11_500,
            expiry: 1718400000,
            chainId: U256::from(chain_id),
        };

        // Sign with key1
        let signature = sign_quote(&quote, chain_id, contract_address, &private_key1).unwrap();

        // Try to verify with key2's address (should fail)
        let is_valid =
            verify_quote_signature(&quote, chain_id, contract_address, signature, wrong_signer)
                .unwrap();

        assert!(!is_valid, "Signature from wrong signer should fail");
    }

    #[test]
    fn test_eip712_tampered_quote_fails() {
        use k256::ecdsa::SigningKey;
        use rand::RngCore;

        let mut private_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut private_key);
        private_key[0] |= 1;
        let signing_key = SigningKey::from_bytes((&private_key).into()).unwrap();
        let signer_address = public_key_to_address(signing_key.verifying_key());

        let chain_id = 421614u64;
        let contract_address = address!("0x4444444444444444444444444444444444444444");

        let original_quote = ExecutionQuote {
            assetId: 1,
            targetContract: contract_address,
            baseGasFee: 20_000,
            relayerMarkupFee: 3_000,
            totalExecutionFee: 23_000,
            expiry: 1718500000,
            chainId: U256::from(chain_id),
        };

        // Sign the original quote
        let signature =
            sign_quote(&original_quote, chain_id, contract_address, &private_key).unwrap();

        // Tamper with the quote (increase totalExecutionFee)
        let mut tampered_quote = original_quote.clone();
        tampered_quote.totalExecutionFee = 50_000_000; // 10x higher!

        // Try to verify the tampered quote with the original signature
        let is_valid = verify_quote_signature(
            &tampered_quote,
            chain_id,
            contract_address,
            signature,
            signer_address,
        )
        .unwrap();

        assert!(!is_valid, "Tampered quote should fail verification");
    }

    #[test]
    fn test_eip712_quote_hash_binds_asset_id() {
        use k256::ecdsa::SigningKey;
        use rand::RngCore;

        let mut private_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut private_key);
        private_key[0] |= 1;
        let signing_key = SigningKey::from_bytes((&private_key).into()).unwrap();
        let signer_address = public_key_to_address(signing_key.verifying_key());

        let chain_id = 421614u64;
        let contract_address = address!("0xabcdef1234567890abcdef1234567890abcdef12");

        // Valid WETH quote (assetId = 0)
        let weth_quote = ExecutionQuote {
            assetId: 0,
            targetContract: contract_address,
            baseGasFee: 15_000,
            relayerMarkupFee: 2_250,
            totalExecutionFee: 17_250,
            expiry: 1718234567,
            chainId: U256::from(chain_id),
        };

        // Sign WETH quote
        let signature = sign_quote(&weth_quote, chain_id, contract_address, &private_key).unwrap();

        // 1. Valid signature verifies for original quote
        let is_valid = verify_quote_signature(
            &weth_quote,
            chain_id,
            contract_address,
            signature,
            signer_address,
        )
        .unwrap();
        assert!(is_valid, "Valid WETH quote signature must verify");

        // 2. Tampered quote with assetId = 1 (USDC)
        let mut tampered_quote = weth_quote.clone();
        tampered_quote.assetId = 1;

        // Signature must fail for tampered quote
        let is_valid_tampered = verify_quote_signature(
            &tampered_quote,
            chain_id,
            contract_address,
            signature,
            signer_address,
        )
        .unwrap();
        assert!(
            !is_valid_tampered,
            "Tampering asset_id must fail signature verification"
        );

        // 3. Circuit quote hash must differ between asset_id 0 and 1
        let hash_weth = compute_quote_hash(&weth_quote, chain_id, contract_address);
        let hash_tampered = compute_quote_hash(&tampered_quote, chain_id, contract_address);
        assert_ne!(
            hash_weth, hash_tampered,
            "Asset ID tampering must produce different signing hashes"
        );

        let (hi_weth, lo_weth) = zeltra_core::split_quote_hash_to_limbs(&hash_weth.0);
        let (hi_tampered, lo_tampered) = zeltra_core::split_quote_hash_to_limbs(&hash_tampered.0);
        assert!(
            hi_weth != hi_tampered || lo_weth != lo_tampered,
            "Circuit quote hash limbs must be invalidated when asset_id is tampered"
        );
    }
}
