# AstraEngine

**AstraEngine** is an independent, Windows-first game engine written primarily in Rust. The goal is a standalone editor and a separate game runtime, with a professional 3D workflow inspired by modern game-development tools.

> Early development. The current repository establishes the Rust workspace and engine boundaries; it is not yet a usable 3D editor.

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

The current editor executable is a bootstrap entry point. A native desktop window, dockable panels and the 3D viewport are subsequent milestones.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Development roadmap](docs/ROADMAP.md)
