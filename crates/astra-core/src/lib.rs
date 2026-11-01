//! Foundational types shared by AstraEngine crates.

/// Public engine identity and initial release version.
pub const ENGINE_NAME: &str = "AstraEngine";
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Basic application configuration shared by editor and runtime hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    pub application_name: String,
    pub target_fps: u32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            application_name: ENGINE_NAME.to_owned(),
            target_fps: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_a_valid_name_and_frame_rate() {
        let config = EngineConfig::default();
        assert_eq!(config.application_name, ENGINE_NAME);
        assert!(config.target_fps > 0);
    }
}
