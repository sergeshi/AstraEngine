# Development Roadmap

## Milestone 0 — Repository foundation
- [x] Create independent Rust workspace.
- [x] Define crate boundaries and product architecture.
- [x] Add minimal, testable core/scene/renderer/runtime APIs.
- [x] Add a standalone editor bootstrap executable.
- [ ] Verify build on the user's Windows machine.

## Milestone 1 — Native application shell
- [ ] Validate `winit` + `egui` + `egui_dock` on Windows.
- [ ] Create standalone editor window and dockable panel layout.
- [ ] Add project create/open workflow and persistent user settings.
- [ ] Add logging, panic reporting and application diagnostics.

## Milestone 2 — Project and scene foundation
- [ ] Versioned project manifest and safe project creation.
- [ ] Stable entity identifiers and entity hierarchy.
- [ ] Transform editing and parent-child relationships.
- [ ] Undo/redo command history and scene serialization.

## Milestone 3 — 3D viewport
- [ ] GPU adapter/device and window surface initialization.
- [ ] Camera navigation, grid and world axes.
- [ ] Render a test mesh with depth and basic lighting.
- [ ] Selection and translate/rotate/scale gizmos.

## Milestone 4 — Assets and content browser
- [ ] Asset database and import metadata.
- [ ] glTF mesh, material and texture import.
- [ ] Content Browser, thumbnails and dependency tracking.
- [ ] Drag-and-drop assets into the scene.

## Milestone 5 — Production editor workflow
- [ ] World Outliner, Details/Inspector, viewport and console.
- [ ] Materials, lighting and environment tools.
- [ ] Prefab/scene instancing workflow.
- [ ] Save/reload and recovery from invalid project data.

## Milestone 6 — Runtime and gameplay
- [ ] Launch a project without the editor.
- [ ] Input, physics, audio and animation integration.
- [ ] Gameplay scripting and visual scripting foundation.
- [ ] Play-in-editor and runtime debugging.

## Milestone 7 — Advanced world tools
- [ ] Terrain and foliage workflow.
- [ ] Animation state machines and skeletal tools.
- [ ] Navigation and AI services.
- [ ] Profiling and performance diagnostics.

## Milestone 8 — Packaging
- [ ] Windows game build pipeline.
- [ ] Asset cooking and reproducible release artifacts.
- [ ] Runtime deployment and project templates.
