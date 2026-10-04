//! Input validation and safety bounds enforcement (DEC-019)
//!
//! Validates client requests before database locks, queueing, or broadcast:
//! - EVM recipient address format & non-zero address check
//! - Spend amount bounds: min 5 USDC (on-chain limit) to max limit (circuit breaker)
//! - Cross-chain params: allowlisted CCIP chain selectors & valid destination contract

use alloy::primitives::Address;
use std::collections::HashSet;
use std::str::FromStr;

/// On-chain Stylus minimum spend amount is 5 USDC (5_000_000 micro-USDC).
pub const MIN_SPEND_AMOUNT_USDC: u64 = 5_000_000;

/// Default maximum spend per transaction (50,000 USDC) circuit breaker.
pub const DEFAULT_MAX_SPEND_AMOUNT_USDC: u64 = 50_000_000_000;

/// Known Chainlink CCIP Chain Selectors (Testnets and Mainnets)
pub const CCIP_ARBITRUM_SEPOLIA: u64 = 3478487238524512106;
pub const CCIP_ETHEREUM_SEPOLIA: u64 = 16015286601757825753;
pub const CCIP_BASE_SEPOLIA: u64 = 10344971235874465080;
pub const CCIP_OPTIMISM_SEPOLIA: u64 = 5224473277236331295;

/// Canonical list of known sanctioned digital currency addresses (OFAC SDN List).
/// Implements DEC-026 Layer 3: Relayer Operational Policy (Low-Cost Sanctions Screening via Public Data).
pub static KNOWN_SANCTIONED_ADDRESSES: &[&str] = &[
    "0x8576acc5c05d6ce88f4e49bf65bdf0c62f91353c", // Tornado Cash: Router
    "0xd90e2f925da726b50c4ed8d0fb90ad053324f31b", // Tornado Cash: Core Contract
    "0xd7aa9749da3e9ae6008b8b0e8cecd25a666986cf", // OFAC SDN - Lazarus Group
    "0x098b716b8aaf21512996dc57eb0615e2383e2f96", // Tornado Cash 0.1 ETH
    "0xa160cdab225685da1d56aa342ad8841c3b53f291", // Tornado Cash 1 ETH
    "0x12d66f87a04a9e220743712ce6d9bb1b5616b8fc", // Tornado Cash 10 ETH
    "0x47ce0c6ed5b0ce3d3a51fdb1c52dc66a7c3c2936", // Tornado Cash 100 ETH
];

/// Configurable safety limits for spend operations
#[derive(Clone, Debug)]
pub struct SafetyLimits {
    pub min_spend_usdc: u64,
    pub max_spend_usdc: u64,
    pub allowed_chain_selectors: HashSet<u64>,
    pub sanctioned_addresses: HashSet<Address>,
}

impl SafetyLimits {
    pub fn is_sanctioned(&self, address: &Address) -> bool {
        self.sanctioned_addresses.contains(address)
    }
}

impl Default for SafetyLimits {
    fn default() -> Self {
        let min_spend_usdc = std::env::var("NIMBUS_MIN_SPEND_USDC")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(MIN_SPEND_AMOUNT_USDC);

        let max_spend_usdc = std::env::var("NIMBUS_MAX_SPEND_USDC")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_MAX_SPEND_AMOUNT_USDC);

        let mut allowed_chain_selectors = HashSet::new();
        if let Ok(selectors_str) = std::env::var("NIMBUS_ALLOWED_CHAIN_SELECTORS") {
            for part in selectors_str.split(',') {
                if let Ok(sel) = part.trim().parse::<u64>() {
                    allowed_chain_selectors.insert(sel);
                }
            }
        }

        // Always include canonical CCIP testnet selectors if empty
        if allowed_chain_selectors.is_empty() {
            allowed_chain_selectors.insert(CCIP_ARBITRUM_SEPOLIA);
            allowed_chain_selectors.insert(CCIP_ETHEREUM_SEPOLIA);
            allowed_chain_selectors.insert(CCIP_BASE_SEPOLIA);
            allowed_chain_selectors.insert(CCIP_OPTIMISM_SEPOLIA);
        }

        let mut sanctioned_addresses = HashSet::new();
        for addr_str in KNOWN_SANCTIONED_ADDRESSES {
            if let Ok(addr) = Address::from_str(addr_str) {
                sanctioned_addresses.insert(addr);
            }
        }
        if let Ok(custom_sanctioned) = std::env::var("NIMBUS_SANCTIONED_ADDRESSES") {
            for part in custom_sanctioned.split(',') {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    if let Ok(addr) = Address::from_str(trimmed) {
                        sanctioned_addresses.insert(addr);
                    }
                }
            }
        }

        Self {
            min_spend_usdc,
            max_spend_usdc,
            allowed_chain_selectors,
            sanctioned_addresses,
        }
    }
}

/// Validation errors for spend requests
#[derive(Debug, PartialEq, Eq)]
pub enum ValidationError {
    InvalidRecipientAddress(String),
    ZeroRecipientAddress,
    RecipientSanctioned(String),
    AmountTooSmall { amount: u64, min_required: u64 },
    AmountTooLarge { amount: u64, max_allowed: u64 },
    InvalidDestinationContract(String),
    ZeroDestinationContract,
    UnsupportedChainSelector(u64),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRecipientAddress(err) => write!(f, "Invalid recipient address: {}", err),
            Self::ZeroRecipientAddress => write!(f, "Recipient address cannot be zero address"),
            Self::RecipientSanctioned(addr) => write!(
                f,
                "Recipient address {} is restricted under public sanctions policy (DEC-026)",
                addr
            ),
            Self::AmountTooSmall {
                amount,
                min_required,
            } => write!(
                f,
                "Spend amount {} is below minimum required {}",
                amount, min_required
            ),
            Self::AmountTooLarge {
                amount,
                max_allowed,
            } => write!(
                f,
                "Spend amount {} exceeds maximum allowed {}",
                amount, max_allowed
            ),
            Self::InvalidDestinationContract(err) => {
                write!(f, "Invalid cross-chain destination contract: {}", err)
            }
            Self::ZeroDestinationContract => {
                write!(f, "Cross-chain destination contract cannot be zero address")
            }
            Self::UnsupportedChainSelector(sel) => {
                write!(f, "Unsupported cross-chain destination selector: {}", sel)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate spend request recipient, amount, and cross-chain routing
pub fn validate_spend_request(
    recipient: &str,
    amount: u64,
    cross_chain: Option<&crate::dto::CrossChainParams>,
    limits: &SafetyLimits,
) -> Result<Address, ValidationError> {
    // 1. Recipient address validation (EVM format and non-zero)
    let parsed_recipient = Address::from_str(recipient)
        .map_err(|e| ValidationError::InvalidRecipientAddress(e.to_string()))?;

    if parsed_recipient == Address::ZERO {
        return Err(ValidationError::ZeroRecipientAddress);
    }

    if limits.is_sanctioned(&parsed_recipient) {
        return Err(ValidationError::RecipientSanctioned(
            parsed_recipient.to_string(),
        ));
    }

    // 2. Amount safety bounds (Min & Max)
    if amount < limits.min_spend_usdc {
        return Err(ValidationError::AmountTooSmall {
            amount,
            min_required: limits.min_spend_usdc,
        });
    }

    if amount > limits.max_spend_usdc {
        return Err(ValidationError::AmountTooLarge {
            amount,
            max_allowed: limits.max_spend_usdc,
        });
    }

    // 3. Cross-chain parameters validation (if present)
    if let Some(cc) = cross_chain {
        if !limits
            .allowed_chain_selectors
            .contains(&cc.destination_chain_selector)
        {
            return Err(ValidationError::UnsupportedChainSelector(
                cc.destination_chain_selector,
            ));
        }

        let parsed_dest = Address::from_str(&cc.destination_contract)
            .map_err(|e| ValidationError::InvalidDestinationContract(e.to_string()))?;

        if parsed_dest == Address::ZERO {
            return Err(ValidationError::ZeroDestinationContract);
        }
    }

    Ok(parsed_recipient)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::CrossChainParams;

    #[test]
    fn test_valid_spend_request() {
        let limits = SafetyLimits::default();
        let recipient = "0x1111111111111111111111111111111111111111";
        let amount = 10_000_000; // 10 USDC

        let res = validate_spend_request(recipient, amount, None, &limits);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), Address::from_str(recipient).unwrap());
    }

    #[test]
    fn test_reject_zero_recipient_address() {
        let limits = SafetyLimits::default();
        let recipient = "0x0000000000000000000000000000000000000000";
        let amount = 10_000_000;

        let res = validate_spend_request(recipient, amount, None, &limits);
        assert_eq!(res, Err(ValidationError::ZeroRecipientAddress));
    }

    #[test]
    fn test_reject_malformed_recipient_address() {
        let limits = SafetyLimits::default();
        let amount = 10_000_000;

        // Truncated
        let res1 = validate_spend_request("0x1234", amount, None, &limits);
        assert!(matches!(
            res1,
            Err(ValidationError::InvalidRecipientAddress(_))
        ));

        // Non-hex characters
        let res2 = validate_spend_request(
            "0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
            amount,
            None,
            &limits,
        );
        assert!(matches!(
            res2,
            Err(ValidationError::InvalidRecipientAddress(_))
        ));
    }

    #[test]
    fn test_reject_amount_below_min() {
        let limits = SafetyLimits::default();
        let recipient = "0x1111111111111111111111111111111111111111";

        // 0 USDC
        let res0 = validate_spend_request(recipient, 0, None, &limits);
        assert_eq!(
            res0,
            Err(ValidationError::AmountTooSmall {
                amount: 0,
                min_required: MIN_SPEND_AMOUNT_USDC
            })
        );

        // 4.99 USDC (less than 5 USDC minimum)
        let res_dust = validate_spend_request(recipient, 4_999_999, None, &limits);
        assert_eq!(
            res_dust,
            Err(ValidationError::AmountTooSmall {
                amount: 4_999_999,
                min_required: MIN_SPEND_AMOUNT_USDC
            })
        );
    }

    #[test]
    fn test_reject_amount_above_max() {
        let limits = SafetyLimits::default();
        let recipient = "0x1111111111111111111111111111111111111111";

        // 50,001 USDC
        let excessive_amount = 50_001_000_000;
        let res = validate_spend_request(recipient, excessive_amount, None, &limits);
        assert_eq!(
            res,
            Err(ValidationError::AmountTooLarge {
                amount: excessive_amount,
                max_allowed: DEFAULT_MAX_SPEND_AMOUNT_USDC
            })
        );
    }

    #[test]
    fn test_cross_chain_validation() {
        let limits = SafetyLimits::default();
        let recipient = "0x1111111111111111111111111111111111111111";
        let amount = 10_000_000;

        // Valid CCIP params (Base Sepolia)
        let valid_cc = CrossChainParams {
            destination_chain_selector: CCIP_BASE_SEPOLIA,
            destination_contract: "0x2222222222222222222222222222222222222222".to_string(),
        };
        assert!(validate_spend_request(recipient, amount, Some(&valid_cc), &limits).is_ok());

        // Unsupported selector
        let invalid_sel_cc = CrossChainParams {
            destination_chain_selector: 9999999999,
            destination_contract: "0x2222222222222222222222222222222222222222".to_string(),
        };
        assert_eq!(
            validate_spend_request(recipient, amount, Some(&invalid_sel_cc), &limits),
            Err(ValidationError::UnsupportedChainSelector(9999999999))
        );

        // Zero destination contract
        let zero_dest_cc = CrossChainParams {
            destination_chain_selector: CCIP_BASE_SEPOLIA,
            destination_contract: "0x0000000000000000000000000000000000000000".to_string(),
        };
        assert_eq!(
            validate_spend_request(recipient, amount, Some(&zero_dest_cc), &limits),
            Err(ValidationError::ZeroDestinationContract)
        );

        // Malformed destination contract
        let malformed_dest_cc = CrossChainParams {
            destination_chain_selector: CCIP_BASE_SEPOLIA,
            destination_contract: "not-an-address".to_string(),
        };
        assert!(matches!(
            validate_spend_request(recipient, amount, Some(&malformed_dest_cc), &limits),
            Err(ValidationError::InvalidDestinationContract(_))
        ));
    }

    #[test]
    fn test_reject_sanctioned_recipient_address() {
        let limits = SafetyLimits::default();
        // Tornado Cash router address
        let sanctioned_recipient = "0x8576acc5c05d6ce88f4e49bf65bdf0c62f91353c";
        let amount = 10_000_000;

        let res = validate_spend_request(sanctioned_recipient, amount, None, &limits);
        assert!(matches!(res, Err(ValidationError::RecipientSanctioned(_))));

        // Lazarus Group OFAC address
        let lazarus_recipient = "0xd7aa9749da3e9ae6008b8b0e8cecd25a666986cf";
        let res2 = validate_spend_request(lazarus_recipient, amount, None, &limits);
        assert!(matches!(res2, Err(ValidationError::RecipientSanctioned(_))));

        // Clean recipient passes
        let clean_recipient = "0x1111111111111111111111111111111111111111";
        assert!(validate_spend_request(clean_recipient, amount, None, &limits).is_ok());
    }
}
