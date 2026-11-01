//! Game runtime lifecycle, independent from the editor application.

use astra_core::EngineConfig;
use astra_renderer::{RendererConfig, RendererState};
use astra_scene::Scene;

/// Runtime lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Created,
    Running,
    Paused,
    Stopped,
}

/// A minimal runtime host. Platform, window and GPU services are added in later milestones.
#[derive(Debug)]
pub struct GameRuntime {
    pub config: EngineConfig,
    pub renderer_config: RendererConfig,
    pub scene: Scene,
    state: RuntimeState,
    renderer_state: RendererState,
}

impl Default for GameRuntime {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

impl GameRuntime {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            renderer_config: RendererConfig::default(),
            scene: Scene::new(),
            state: RuntimeState::Created,
            renderer_state: RendererState::NotInitialized,
        }
    }

    pub fn state(&self) -> RuntimeState {
        self.state
    }

    pub fn renderer_state(&self) -> RendererState {
        self.renderer_state
    }

    pub fn start(&mut self) -> Result<(), RuntimeState> {
        match self.state {
            RuntimeState::Created | RuntimeState::Paused => {
                self.state = RuntimeState::Running;
                Ok(())
            }
            other => Err(other),
        }
    }

    pub fn pause(&mut self) -> Result<(), RuntimeState> {
        if self.state != RuntimeState::Running {
            return Err(self.state);
        }
        self.state = RuntimeState::Paused;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.state = RuntimeState::Stopped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_lifecycle_rejects_invalid_transitions() {
        let mut runtime = GameRuntime::default();
        assert_eq!(runtime.state(), RuntimeState::Created);
        assert_eq!(runtime.start(), Ok(()));
        assert_eq!(runtime.pause(), Ok(()));
        assert_eq!(runtime.start(), Ok(()));
        runtime.stop();
        assert_eq!(runtime.state(), RuntimeState::Stopped);
        assert_eq!(runtime.start(), Err(RuntimeState::Stopped));
    }
}
