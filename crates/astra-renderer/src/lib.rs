//! Rendering contracts. No GPU resources are created in this bootstrap milestone.

/// Graphics backend selected by a future renderer implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsBackend {
    /// Direct3D 12 is the preferred Windows backend candidate.
    Direct3D12,
    /// Vulkan is a secondary backend candidate.
    Vulkan,
    /// Used by tests and headless tools that do not create a GPU device.
    Headless,
}

/// Requested renderer configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererConfig {
    pub backend: GraphicsBackend,
    pub vertical_sync: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            backend: GraphicsBackend::Direct3D12,
            vertical_sync: true,
        }
    }
}

/// Renderer lifecycle state exposed to editor/runtime diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererState {
    NotInitialized,
    Ready,
    Suspended,
    DeviceLost,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_default_prefers_direct3d12() {
        assert_eq!(RendererConfig::default().backend, GraphicsBackend::Direct3D12);
    }
}
