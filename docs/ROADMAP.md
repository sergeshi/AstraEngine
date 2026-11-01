# Development Roadmap

## Milestone 0 — Repository foundation
- [x] Create independent Rust workspace.
- [x] Define crate boundaries and product architecture.
- [x] Add minimal, testable core/scene/renderer/runtime APIs.
- [x] Add a standalone editor bootstrap executable.
- [x] Verify workspace compilation and unit tests on Windows.

## Milestone 1 — Native application shell (Editor 0.2)
- [x] Add a native desktop window through eframe/winit.
- [x] Integrate egui and egui_dock.
- [x] Add Hierarchy, Viewport preview, Inspector, Content Browser and Console panels.
- [x] Add menu bar, toolbar, resizable/dockable panels and reset-layout command.
- [x] Persist dock layout and basic editor state between launches.
- [ ] Verify native editor build and interaction on the user's Windows machine.
- [ ] Add project create/open workflow, logging and panic reporting.

## Milestone 2 — Project and scene foundation
- [ ] Versioned project manifest and safe project creation.
- [ ] Stable entity identifiers and entity hierarchy.
- [ ] Transform editing connected to the scene data model.
- [ ] Undo/redo command history and scene serialization.

## Milestone 3 — 3D viewport and renderer
- [ ] GPU adapter/device and window surface initialization.
- [ ] Camera navigation, grid and world axes rendered by the GPU.
- [ ] Render a test mesh with depth and basic lighting.
- [ ] Selection and translate/rotate/scale gizmos.

## Milestone 4 — Assets and content browser
- [ ] Asset database and import metadata.
- [ ] glTF mesh, material and texture import.
- [ ] Content Browser file operations, thumbnails and dependency tracking.
- [ ] Drag-and-drop assets into the scene.

## Milestone 5 — Production editor workflow
- [ ] World Outliner, Details/Inspector, viewport and console connected to engine data.
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
