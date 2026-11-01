# AstraEngine

**AstraEngine** is an independent, Windows-first game engine written primarily in Rust. Its goal is a standalone editor and a separate game runtime, with a professional 3D workflow inspired by the capabilities and usability of modern engines.

> Early development: APIs and project layout are expected to evolve.

## Project principles

- Rust-first engine and tooling.
- Standalone editor; the editor is not a modified Godot build.
- Runtime can launch games without the editor.
- Modular crates with explicit boundaries.
- Small, testable increments; no placeholder claims of completed features.
- No source code is copied from Narrative Studio.

## Workspace

| Crate | Responsibility |
|---|---|
| `astra-core` | Engine identity, configuration, lifecycle primitives |
| `astra-scene` | Scene graph and transform model |
| `astra-renderer` | Rendering contracts and backend boundary |
| `astra-runtime` | Game runtime host |
| `astra-editor` | Standalone editor executable |

## Build

Install the stable Rust toolchain and the Windows C++ build prerequisites required by Rust dependencies. Then run:

```powershell
cargo test --workspace
cargo run -p astra-editor
```

The initial milestone establishes the workspace and a minimal editor entry point. Rendering and interactive editor panels will be added in subsequent milestones.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Development roadmap](docs/ROADMAP.md)
