//! Shared identity and configuration primitives for AstraEngine.

/// Public engine identity and version information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub target_platform: &'static str,
}

impl EngineInfo {
    pub const CURRENT: Self = Self {
        name: "AstraEngine",
        version: env!("CARGO_PKG_VERSION"),
        target_platform: "Windows",
    };
}

/// Configuration shared by editor and runtime hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    pub application_name: String,
    pub fixed_timestep_hz: u32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            application_name: EngineInfo::CURRENT.name.to_owned(),
            fixed_timestep_hz: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_a_valid_fixed_timestep() {
        let config = EngineConfig::default();
        assert_eq!(config.application_name, "AstraEngine");
        assert!(config.fixed_timestep_hz > 0);
    }
}
