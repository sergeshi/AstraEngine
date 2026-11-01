# AstraEngine Architecture

## 1. Product definition

AstraEngine is a new, standalone game engine and editor. The first supported development platform is Windows. The editor and game runtime are separate applications built from shared Rust crates.

The long-term product target is a cohesive 3D development environment: scene viewport, hierarchy/outliner, property inspector, asset browser, material and lighting tools, animation, physics, visual gameplay logic, project packaging and a distributable runtime.

This is a staged engineering target, not a claim that these systems already exist.

## 2. Architectural principles

1. **Rust-first:** engine-owned systems and tools are implemented in Rust.
2. **Standalone:** AstraEditor is its own desktop application. It is not a fork or reskin of Godot.
3. **Runtime/editor separation:** game builds must not require the editor to run.
4. **Explicit boundaries:** crates communicate through stable data contracts rather than reaching into each other's internals.
5. **Data-driven projects:** scenes, assets, settings and build configuration will use documented, versioned formats.
6. **Incremental verification:** every milestone must compile and have automated tests before expanding.
7. **No borrowed project code:** no source code or architecture is copied from Narrative Studio.

## 3. Workspace crate map

```text
AstraEngine/
  crates/
    astra-core/       shared identity, config, errors and primitives
    astra-scene/      entities, transforms, hierarchy and scene serialization
    astra-renderer/   rendering API and graphics backend integration
    astra-runtime/    game loop, world execution and platform services
    astra-editor/     standalone desktop editor executable
  docs/
    ARCHITECTURE.md
    ROADMAP.md
```

### Dependency direction

- `astra-core` is the lowest-level shared crate.
- `astra-scene` depends on core.
- `astra-renderer` depends on core and later consumes scene render data.
- `astra-runtime` composes core, scene and renderer.
- `astra-editor` is the application layer and composes engine/editor services.

Lower-level crates must not depend on the editor.

## 4. Planned technology choices

| Area | Technology | Role |
|---|---|---|
| Native desktop window and event loop | `winit` (through `eframe`) | Windows application window and input events |
| Editor UI | `egui` / `eframe` | Rust-native immediate-mode editor interface |
| Docking | `egui_dock` | Resizable, movable and detachable editor panels |
| GPU abstraction | `wgpu` | Future 3D rendering backend; not yet connected to the editor viewport |
| Math | `glam` | Vector, matrix and transform types |
| Scene/ECS | Custom scene model first; evaluate `bevy_ecs` when gameplay systems arrive | Keep early scene semantics explicit |
| Physics | `rapier3d` | Physics candidate; integrate after scene/runtime contracts stabilize |
| 3D asset interchange | glTF 2.0 | Mesh, material, skin and animation interchange |

No Godot or Blender source code is required for this architecture. Godot can be studied as a reference and selected compatible libraries may be integrated after license and technical review. Blender is an external content-creation tool; glTF is the initial interchange path. Blender is GPL-licensed, so copying its source into this project is not part of the plan.

## 5. Core data contracts

### Project

A project has a root directory and a versioned project manifest. It references asset roots, startup scene, engine compatibility and project settings. Opening a project must validate the manifest and report actionable errors instead of silently discarding data.

### Scene

A scene is a serializable collection of stable entity identifiers and components. Transform data uses local translation, rotation and scale. Parent-child relationships must be acyclic. Scene editing belongs to editor services; scene execution belongs to runtime services.

### Assets

Imported assets are source files plus generated/imported artifacts and metadata. The asset database will track stable asset IDs, paths, import settings and dependencies. Generated files must be reproducible and safe to delete/rebuild.

### Runtime

The runtime owns application lifecycle, world activation, input, time-step policy and service startup/shutdown. It must not depend on editor UI code.

## 6. Rendering plan

The renderer is a separate subsystem behind a backend boundary. The first real graphics milestone will create a window surface, select a supported adapter, configure a swapchain/surface and render a clear color. Subsequent milestones add a camera, depth buffer, grid, mesh buffers, materials, lighting and editor selection overlays.

The initial backend candidate is `wgpu`. Backend choice and required Windows graphics features will be validated on the target machine.

## 7. Editor layout and current shell

Editor 0.2 uses `eframe` for the native Windows application and `egui_dock` for movable, resizable and detachable panels. The initial workspace contains:

- Hierarchy: sample scene objects and current selection.
- Viewport: a 2D grid/axis placeholder, not yet a 3D GPU viewport.
- Inspector: editable prototype transform fields.
- Content Browser: placeholder asset categories, not yet connected to disk.
- Console: in-editor log output.

The menu bar includes File/Edit/Window/Help, and the toolbar exposes prototype New/Save/Play/Stop commands. Commands that depend on project serialization or runtime launch are explicitly marked as not implemented. Dock layout and basic editor state are persisted through eframe storage.

The panels are a UI shell only. Scene entities, transforms, asset indexing, undo/redo, project files and GPU rendering will be connected in later milestones.

## 8. Serialization and compatibility

Persistent formats must carry explicit schema versions. Loading should distinguish unsupported versions, malformed data and missing assets. Migrations must be explicit and tested. The editor must never overwrite a project file until a validated replacement has been written safely.

## 9. Testing strategy

- Unit tests for identifiers, transforms, hierarchy and runtime state transitions.
- Integration tests for project and scene serialization once introduced.
- Windows CI for formatting, linting and workspace tests.
- Graphics smoke tests on real hardware when the renderer is introduced.
- Manual acceptance checks for editor interaction, input and asset workflows.
