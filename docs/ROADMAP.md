# Development Roadmap

## Milestone 0 — Repository foundation
- [x] Create independent Rust workspace.
- [x] Define crate boundaries and product architecture.
- [x] Add a minimal editor executable and testable core primitives.
- [ ] Verify build on the user's Windows machine.

## Milestone 1 — Application shell
- [ ] Select and integrate the desktop window/UI stack.
- [ ] Create the standalone editor window and dockable panel layout.
- [ ] Add project create/open workflow and persistent user settings.

## Milestone 2 — Scene foundation
- [ ] Entity hierarchy and stable identifiers.
- [ ] Transform editing and parent-child relationships.
- [ ] Undo/redo command history and scene serialization.

## Milestone 3 — 3D viewport
- [ ] GPU device and swapchain/surface initialization.
- [ ] Camera navigation, grid, axes, selection and transform gizmos.
- [ ] Render a test mesh with depth and basic lighting.

## Milestone 4 — Assets and content browser
- [ ] Project asset database and import metadata.
- [ ] glTF model and texture import.
- [ ] Content browser, thumbnails and dependency tracking.

## Milestone 5 — Production editor workflow
- [ ] World Outliner, Details/Inspector, viewport and console.
- [ ] Materials, lighting, environment and prefab workflow.
- [ ] Save/reload and robust recovery from invalid project data.

## Milestone 6 — Runtime and gameplay
- [ ] Launch project without editor.
- [ ] Input, physics, audio and animation integration.
- [ ] Visual scripting and gameplay framework.

## Milestone 7 — Packaging
- [ ] Windows game build pipeline.
- [ ] Asset cooking and reproducible release artifacts.
