#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMode {
    Development,
    Test,
    HardTest,
    Production,
    Mainnet,
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
