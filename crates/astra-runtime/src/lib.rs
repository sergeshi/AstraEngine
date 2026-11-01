//! Runtime host primitives; this crate intentionally has no editor dependency.

use astra_core::EngineConfig;

/// Runtime host configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub engine: EngineConfig,
    pub fixed_update_hz: u32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            engine: EngineConfig::default(),
            fixed_update_hz: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_uses_a_positive_fixed_update_rate() {
        assert!(RuntimeConfig::default().fixed_update_hz > 0);
    }
}
