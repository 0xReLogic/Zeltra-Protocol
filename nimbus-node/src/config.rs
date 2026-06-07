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
        let bind_addr = std::env::var("NIMBUS_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1".to_string());

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
        self.require_tailscale || self.bind_addr.starts_with("100.") || self.bind_addr.starts_with("fd7a:")
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
}
