# AstraEngine

**AstraEngine** is an independent, Windows-first game engine written primarily in Rust. The goal is a standalone editor and a separate game runtime, with a professional 3D workflow inspired by modern game-development tools.

> Early development. The repository contains a native desktop editor with a scene hierarchy, inspector, dockable panels, OpenGL viewport, transform gizmos, project persistence and 3D asset import. Rendering and asset workflows are still evolving.

## Principles

- Rust-first engine and tools.
- Standalone editor; not a modified Godot build.
- Runtime can eventually launch games without the editor.
- Modular crates with explicit boundaries.
- Small, testable increments; no placeholder claims of completed features.
- No source code is copied from Narrative Studio.

## Workspace

| Crate | Responsibility |
|---|---|
| `astra-core` | Engine identity, configuration and shared primitives |
| `astra-scene` | Scene graph, entity identifiers and transforms |
| `astra-renderer` | Rendering contracts and backend boundary |
| `astra-runtime` | Game runtime lifecycle |
| `astra-editor` | Standalone editor executable |

## Build on Windows

Install the stable Rust toolchain and the Visual Studio C++ build tools required by native dependencies. From the repository directory:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo run -p astra-editor
```

The editor opens a native desktop window with Hierarchy, Viewport, Inspector, Content Browser and Console panels. The viewport supports primitive and imported meshes, camera navigation, selection, transform gizmos, normals-based lighting, base-color textures and metallic-roughness PBR shading.

## 3D assets and materials

- Import OBJ, glTF/GLB and FBX meshes from the viewport toolbar.
- OBJ imports triangulate faces and preserve per-corner position/UV/normal indexing; supported MTL factors and texture maps are copied into project assets.
- glTF/GLB imports preserve node transforms, normals, UV0, base-color and metallic-roughness factors/maps.
- FBX imports triangulated geometry, normals, UVs and available PBR material maps.
- Texture resources are copied under `Assets/Textures`; `.astra` stores project-relative references and embedded mesh geometry.
- The Inspector assigns base-color and metallic-roughness maps; Content Browser lists texture and reusable material assets.
- Save As copies registered textures to the destination project directory.

The viewport currently uses a single directional light and a compact metallic-roughness Cook-Torrance shader; it is not yet an image-based-lighting or full production renderer. Imported meshes with multiple material slots are currently represented by the first available material. FBX texture support depends on the data exposed by the source file. Missing or unsupported source attributes are generated where possible.

Run checks before submitting changes:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Development roadmap](docs/ROADMAP.md)
