//! EIP-712 Typed Data Signing for Execution Quotes
//!
//! Implements EIP-712 signing for execution fee quotes to bind users to maximum fees
//! before transaction execution. This prevents relayer overcharging and provides
//! replay protection.

use alloy_primitives::{keccak256, Address, B256};
use alloy_sol_types::{eip712_domain, sol, Eip712Domain, SolStruct};
use serde::{Deserialize, Serialize};

// Define the ExecutionQuote struct using sol! macro
// This auto-generates SolStruct with EIP-712 hashing methods
sol! {
    /// Execution quote that users sign to authorize maximum execution fees
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct ExecutionQuote {
        uint256 quoteId;
        uint256 maxExecutionFee;
        uint256 merchantAmount;
        uint256 quoteExpiry;
        address relayerAddress;
    }
}

/// EIP-712 domain for Nimbus Protocol
pub fn nimbus_domain(chain_id: u64, contract_address: Address) -> Eip712Domain {
    eip712_domain! {
        name: "Nimbus Protocol",
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
    let domain = nimbus_domain(chain_id, contract_address);
    quote.eip712_signing_hash(&domain)
}

/// Compute both the domain separator and struct hash for client verification
pub fn compute_quote_hashes(
    quote: &ExecutionQuote,
    chain_id: u64,
    contract_address: Address,
) -> (B256, B256) {
    let domain = nimbus_domain(chain_id, contract_address);
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

    let signing_key =
        SigningKey::from_bytes(private_key.into()).map_err(|e| format!("Invalid private key: {}", e))?;

    let hash = compute_quote_hash(quote, chain_id, contract_address);

    let (signature, recovery_id) = signing_key
        .sign_prehash_recoverable(&hash.0)
        .map_err(|e| format!("Signing failed: {}", e))?;

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

    let signature = Signature::from_bytes((&sig_bytes).into()).map_err(|e| format!("Invalid signature: {}", e))?;

    let recovered_key = VerifyingKey::recover_from_prehash(&hash.0, &signature, recovery_id)
        .map_err(|e| format!("Recovery failed: {}", e))?;

    let recovered_address = public_key_to_address(&recovered_key);

    // Verify recovered address matches expected signer
    // If recovery succeeded and address matches, the signature is valid
    Ok(recovered_address == expected_signer)
}

/// Convert a public key to an Ethereum address
fn public_key_to_address(public_key: &k256::ecdsa::VerifyingKey) -> Address {
    let public_key_bytes = public_key.to_encoded_point(false);
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
        let quote = ExecutionQuote {
            quoteId: U256::from(123u64),
            maxExecutionFee: U256::from(5_000_000u64),
            merchantAmount: U256::from(100_000_000u64),
            quoteExpiry: U256::from(1718234567u64),
            relayerAddress: address!("0x1234567890123456789012345678901234567890"),
        };

        let chain_id = 421614u64; // Arbitrum Sepolia
        let contract_address = address!("0xabcdef1234567890abcdef1234567890abcdef12");

        let hash1 = compute_quote_hash(&quote, chain_id, contract_address);
        let hash2 = compute_quote_hash(&quote, chain_id, contract_address);

        assert_eq!(hash1, hash2, "Same input should produce same hash");
    }

    #[test]
    fn test_eip712_different_chain_id() {
        let quote = ExecutionQuote {
            quoteId: U256::from(123u64),
            maxExecutionFee: U256::from(5_000_000u64),
            merchantAmount: U256::from(100_000_000u64),
            quoteExpiry: U256::from(1718234567u64),
            relayerAddress: address!("0x1234567890123456789012345678901234567890"),
        };

        let contract_address = address!("0xabcdef1234567890abcdef1234567890abcdef12");

        let hash1 = compute_quote_hash(&quote, 421614u64, contract_address);
        let hash2 = compute_quote_hash(&quote, 1u64, contract_address);

        assert_ne!(hash1, hash2, "Different chain IDs should produce different hashes");
    }

    #[test]
    fn test_eip712_signature_roundtrip() {
        use k256::ecdsa::SigningKey;

        // Generate a random private key for testing
        let private_key_bytes = SigningKey::random(&mut rand::thread_rng()).to_bytes();
        let private_key: [u8; 32] = private_key_bytes.into();

        // Derive the public key and address
        let signing_key = SigningKey::from_bytes((&private_key).into()).unwrap();
        let verifying_key = signing_key.verifying_key();
        let signer_address = public_key_to_address(&verifying_key);

        let quote = ExecutionQuote {
            quoteId: U256::from(456u64),
            maxExecutionFee: U256::from(10_000_000u64),
            merchantAmount: U256::from(200_000_000u64),
            quoteExpiry: U256::from(1718300000u64),
            relayerAddress: address!("0x9876543210987654321098765432109876543210"),
        };

        let chain_id = 421614u64;
        let contract_address = address!("0xfedcba0987654321fedcba0987654321fedcba09");

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

        // Generate two different keys
        let private_key1: [u8; 32] = SigningKey::random(&mut rand::thread_rng()).to_bytes().into();
        let private_key2: [u8; 32] = SigningKey::random(&mut rand::thread_rng()).to_bytes().into();

        let signing_key2 = SigningKey::from_bytes((&private_key2).into()).unwrap();
        let wrong_signer = public_key_to_address(signing_key2.verifying_key());

        let quote = ExecutionQuote {
            quoteId: U256::from(789u64),
            maxExecutionFee: U256::from(3_000_000u64),
            merchantAmount: U256::from(50_000_000u64),
            quoteExpiry: U256::from(1718400000u64),
            relayerAddress: address!("0x1111111111111111111111111111111111111111"),
        };

        let chain_id = 421614u64;
        let contract_address = address!("0x2222222222222222222222222222222222222222");

        // Sign with key1
        let signature = sign_quote(&quote, chain_id, contract_address, &private_key1).unwrap();

        // Try to verify with key2's address (should fail)
        let is_valid = verify_quote_signature(
            &quote,
            chain_id,
            contract_address,
            signature,
            wrong_signer,
        )
        .unwrap();

        assert!(!is_valid, "Signature from wrong signer should fail");
    }

    #[test]
    fn test_eip712_tampered_quote_fails() {
        use k256::ecdsa::SigningKey;

        let private_key: [u8; 32] = SigningKey::random(&mut rand::thread_rng()).to_bytes().into();
        let signing_key = SigningKey::from_bytes((&private_key).into()).unwrap();
        let signer_address = public_key_to_address(signing_key.verifying_key());

        let original_quote = ExecutionQuote {
            quoteId: U256::from(999u64),
            maxExecutionFee: U256::from(5_000_000u64),
            merchantAmount: U256::from(100_000_000u64),
            quoteExpiry: U256::from(1718500000u64),
            relayerAddress: address!("0x3333333333333333333333333333333333333333"),
        };

        let chain_id = 421614u64;
        let contract_address = address!("0x4444444444444444444444444444444444444444");

        // Sign the original quote
        let signature =
            sign_quote(&original_quote, chain_id, contract_address, &private_key).unwrap();

        // Tamper with the quote (increase maxExecutionFee)
        let tampered_quote = ExecutionQuote {
            quoteId: original_quote.quoteId,
            maxExecutionFee: U256::from(50_000_000u64), // 10x higher!
            merchantAmount: original_quote.merchantAmount,
            quoteExpiry: original_quote.quoteExpiry,
            relayerAddress: original_quote.relayerAddress,
        };

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
}
