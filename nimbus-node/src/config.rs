#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMode {
    Development,
    Test,
    HardTest,
    Production,
    Mainnet,
}

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub bind_addr: String,
    pub port: u16,
    pub require_tailscale: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1".to_string(),
            port: 8080,
            require_tailscale: false,
        }
    }
}

impl NetworkConfig {
    pub fn from_env() -> Self {
        let bind_addr =
            std::env::var("NIMBUS_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());

        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);

        let require_tailscale = std::env::var("NIMBUS_REQUIRE_TAILSCALE")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        Self {
            bind_addr,
            port,
            require_tailscale,
        }
    }

    pub fn is_tailscale_bind(&self) -> bool {
        self.require_tailscale
            || self.bind_addr.starts_with("100.")
            || self.bind_addr.starts_with("fd7a:")
    }

    pub fn listener_addr(&self) -> String {
        format!("{}:{}", self.bind_addr, self.port)
    }
}

impl RuntimeMode {
    pub fn from_env() -> Self {
        Self::from_value(&std::env::var("NIMBUS_ENV").unwrap_or_default())
    }

    fn from_value(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "test" => Self::Test,
            "hard-test" | "hard_test" => Self::HardTest,
            "production" | "prod" => Self::Production,
            "mainnet" => Self::Mainnet,
            _ => Self::Development,
        }
    }

    pub fn is_strict(self) -> bool {
        matches!(self, Self::HardTest | Self::Production | Self::Mainnet)
    }

    pub fn is_dev(self) -> bool {
        matches!(self, Self::Development | Self::Test)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::HardTest => "hard-test",
            Self::Production => "production",
            Self::Mainnet => "mainnet",
        }
    }
}

pub fn runtime_mode() -> RuntimeMode {
    RuntimeMode::from_env()
}

/// CCIP destination tracking configuration.
/// All fields are optional — if not set, destination tracking is disabled (graceful no-op).
#[derive(Debug, Clone)]
pub struct CcipDestinationConfig {
    /// RPC URL for the destination chain (e.g., "https://sepolia-rollup.arbitrum.io/rpc")
    pub destination_rpc_url: Option<String>,
    /// CCIP OffRamp contract address on destination chain
    pub ccip_offramp_address: Option<String>,
    /// Timeout before freeing failed nullifier (seconds, default 86400 = 24h)
    pub refund_timeout_secs: u64,
}

impl CcipDestinationConfig {
    pub fn from_env() -> Self {
        let destination_rpc_url = std::env::var("NIMBUS_DESTINATION_RPC_URL").ok();
        let ccip_offramp_address = std::env::var("NIMBUS_CCIP_OFFRAMP").ok();
        let refund_timeout_secs = std::env::var("NIMBUS_CCIP_REFUND_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(86400); // 24 hours default

        Self {
            destination_rpc_url,
            ccip_offramp_address,
            refund_timeout_secs,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.destination_rpc_url.is_some() && self.ccip_offramp_address.is_some()
    }
}

/// Configuration for receipt polling, confirmation depth, and replacement transactions.
#[derive(Debug, Clone)]
pub struct ReceiptFinalityConfig {
    /// Number of block confirmations to wait before marking transaction confirmed (default 1 for Arbitrum)
    pub confirmation_threshold: u64,
    /// Timeout in seconds before replacing stuck mempool transaction (default 30s)
    pub tx_timeout_secs: u64,
    /// Percentage to bump gas price on replacement transaction (default 15%)
    pub gas_bump_percent: u64,
    /// Maximum number of gas bump replacements before stopping (default 3)
    pub max_gas_bumps: u32,
}

impl Default for ReceiptFinalityConfig {
    fn default() -> Self {
        Self {
            confirmation_threshold: 1,
            tx_timeout_secs: 30,
            gas_bump_percent: 15,
            max_gas_bumps: 3,
        }
    }
}

impl ReceiptFinalityConfig {
    pub fn from_env() -> Self {
        let confirmation_threshold = std::env::var("NIMBUS_CONFIRMATION_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1);

        let tx_timeout_secs = std::env::var("NIMBUS_TX_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);

        let gas_bump_percent = std::env::var("NIMBUS_GAS_BUMP_PERCENT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(15);

        let max_gas_bumps = std::env::var("NIMBUS_MAX_GAS_BUMPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(3);

        Self {
            confirmation_threshold,
            tx_timeout_secs,
            gas_bump_percent,
            max_gas_bumps,
        }
    }
}

/// Default Chainlink ETH/USD aggregator on Arbitrum Sepolia
pub const ARBITRUM_SEPOLIA_CHAINLINK_ETH_USD: &str = "0xd30e2101a97dcbAeBCBC04F14C3f624E67A35165";

/// Default Chainlink ETH/USD aggregator on Arbitrum One Mainnet
pub const ARBITRUM_ONE_CHAINLINK_ETH_USD: &str = "0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612";

/// Pricing and oracle configuration for gas reimbursement and margin tracking (DEC-029).
#[derive(Debug, Clone)]
pub struct PricingConfig {
    /// Chainlink ETH/USD price feed aggregator address
    pub eth_feed_address: Option<String>,
    /// Fallback static ETH price in USDC if oracle is unavailable or stale
    pub fallback_eth_price: f64,
    /// Cache TTL for price feed in seconds (default 300s = 5m)
    pub cache_ttl_secs: u64,
    /// Max age of oracle round data before declaring stale (default 3600s = 1 hour)
    pub staleness_threshold_secs: u64,
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            eth_feed_address: Some(ARBITRUM_SEPOLIA_CHAINLINK_ETH_USD.to_string()),
            fallback_eth_price: 3500.0,
            cache_ttl_secs: 300,
            staleness_threshold_secs: 3600,
        }
    }
}

impl PricingConfig {
    pub fn from_env() -> Self {
        let feed = std::env::var("NIMBUS_CHAINLINK_ETH_FEED").ok().or_else(|| {
            // If mainnet mode, default to Arbitrum One; otherwise Sepolia
            if runtime_mode() == RuntimeMode::Mainnet {
                Some(ARBITRUM_ONE_CHAINLINK_ETH_USD.to_string())
            } else {
                Some(ARBITRUM_SEPOLIA_CHAINLINK_ETH_USD.to_string())
            }
        });

        let fallback_eth_price = std::env::var("NIMBUS_ETH_PRICE_USDC")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(3500.0);

        let cache_ttl_secs = std::env::var("NIMBUS_ETH_PRICE_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300);

        let staleness_threshold_secs = std::env::var("NIMBUS_ETH_PRICE_STALENESS_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3600);

        Self {
            eth_feed_address: feed,
            fallback_eth_price,
            cache_ttl_secs,
            staleness_threshold_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_modes_are_explicit() {
        for value in ["hard-test", "production", "mainnet"] {
            assert!(RuntimeMode::from_value(value).is_strict());
        }

        for value in ["development", "dev", "test", ""] {
            assert!(!RuntimeMode::from_value(value).is_strict());
        }
    }

    #[test]
    fn pricing_config_defaults_are_valid() {
        let config = PricingConfig::default();
        assert_eq!(
            config.eth_feed_address.as_deref(),
            Some(ARBITRUM_SEPOLIA_CHAINLINK_ETH_USD)
        );
        assert!((config.fallback_eth_price - 3500.0).abs() < f64::EPSILON);
        assert_eq!(config.cache_ttl_secs, 300);
        assert_eq!(config.staleness_threshold_secs, 3600);
    }
}
