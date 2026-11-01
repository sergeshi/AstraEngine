# AstraEngine Architecture

## 1. Product shape

AstraEngine is a standalone Windows desktop editor paired with an independently launchable game runtime. The editor authors projects and scenes; the runtime loads project data and executes the game without requiring the editor process.

The engine is Rust-first. External projects may be integrated through stable data formats or narrow adapters, but the engine is not a fork of Godot or Blender.

## 2. Workspace boundaries

### astra-core
Foundational identifiers, engine configuration, diagnostics, events, and lifecycle contracts. This crate must not depend on editor or rendering implementation details.

### astra-scene
Scene/document model: entities, hierarchy, names, transforms, components, and serialization boundaries. Scene data must remain usable by both editor and runtime.

### astra-renderer
Rendering abstraction and GPU backend. The first backend is planned around wgpu, with Direct3D 12 on Windows as a primary native path and Vulkan as an alternative where available. Renderer-facing resource handles must not leak backend-specific objects into scene data.

### astra-runtime
Runtime host: project loading, fixed/update frame scheduling, scene activation, input and subsystem orchestration. It must be usable without the editor crate.

### astra-editor
Standalone desktop application. Planned workspace layout: viewport, world outliner, inspector/details, content browser, toolbar, console, and project settings. Editor state and project data are separate.

## 3. Planned subsystem evolution

Additional crates will be introduced when their boundaries become concrete:
- astra-assets: import pipeline, asset database, dependency tracking, hot reload.
- astra-physics: physics integration and collision queries.
- astra-animation: skeletons, clips, state machines, animation graphs.
- astra-audio: spatial audio and mixer.
- astra-scripting: visual graphs and optional text scripting.
- astra-build: packaging, platform targets, and reproducible builds.

## 4. Data flow

1. The editor edits project and scene documents.
2. Asset import creates engine-managed assets and metadata.
3. The runtime loads project settings, assets, and scene documents.
4. The renderer consumes scene snapshots and produces frames.
5. Editor-only tools observe and modify documents through explicit commands.

The runtime must never depend on editor UI state.

## 5. Technology decisions

- Language: Rust, edition 2024.
- Initial platform: Windows 10/11 x64.
- GPU abstraction: wgpu is the leading candidate; backend integration will be validated in a dedicated rendering milestone.
- Windowing and input: a Rust-native desktop/windowing stack will be selected with the first viewport prototype.
- Scene interchange: glTF 2.0 is the initial model interchange target; native project format remains versioned and engine-owned.
- Blender: supported as an external content creation tool through standard formats, not embedded as engine code.
- Godot: may be studied as an architectural reference or interoperated with through standard formats; no Godot source is part of this codebase.

## 6. Engineering rules

- Keep crates acyclic and dependencies pointed toward lower-level abstractions.
- Add tests for model behavior and serialization contracts.
- Avoid unsafe Rust unless a narrowly scoped, reviewed platform boundary makes it unavoidable; workspace currently forbids it.
- Document dependency licenses before adopting external code or assets.
- Every milestone must include build/test instructions and a truthful status report.
