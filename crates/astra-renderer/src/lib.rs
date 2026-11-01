//! Renderer contracts. A concrete GPU backend is introduced in the viewport milestone.

/// Backend-independent description of the rendering surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSurfaceSize {
    pub width: u32,
    pub height: u32,
}

impl RenderSurfaceSize {
    #[must_use]
    pub fn is_visible(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_sized_surface_is_not_visible() {
        assert!(!RenderSurfaceSize { width: 0, height: 720 }.is_visible());
        assert!(RenderSurfaceSize { width: 1280, height: 720 }.is_visible());
    }
}
