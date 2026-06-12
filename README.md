# OpenNTX

[![CI](https://github.com/openntx/openntx/actions/workflows/ci.yml/badge.svg)](https://github.com/openntx/openntx/actions/workflows/ci.yml)
![Status](https://img.shields.io/badge/status-experimental-orange)
![License](https://img.shields.io/badge/license-PolyForm%20Noncommercial%201.0.0-blue)
![Runtime](https://img.shields.io/badge/runtime-not%20implemented%20in%20V0.2-lightgrey)

**Drop EXE. Run Native.**

OpenNTX is an experimental Windows application subsystem for Linux. It aims to make Windows PE/EXE applications feel like native Linux desktop apps by combining PE detection, app manifests, installer capture, sandboxing, desktop integration, and a future NT/Win32 compatibility runtime.

OpenNTX is a source-available project. The default license is noncommercial; commercial use requires explicit written permission from Đặng Nhất Phi. See [LICENSE](LICENSE), [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md), and [CONTRIBUTOR-LICENSE-TERMS.md](CONTRIBUTOR-LICENSE-TERMS.md).

## What OpenNTX Is

- A systems-level foundation for Windows app integration on Linux.
- A manifest-driven model for per-app state, registry overlays, sandbox policy, desktop launchers, and packaging metadata.
- A future home for PE loader, NT runtime, Win32 API, graphics, audio, input, and service-broker research.
- A source-available public codebase for contributors interested in Linux desktop compatibility systems.

## What OpenNTX Is Not

- Not a Wine wrapper.
- Not a Bottles clone.
- Not a Lutris clone.
- Not a VM manager.
- Not a complete Windows runtime in V0.2.
- Not a tool for bypassing DRM, anti-cheat, security controls, or malware analysis safeguards.

## Why This Exists

Linux users should not need to understand prefixes, wrapper scripts, VM setup, random launchers, or compatibility-layer internals just to install a desktop application. The long-term OpenNTX goal is a workflow where Windows applications can be installed, isolated, represented in menus, removed, diagnosed, and packaged with Linux-native conventions.

The target user experience:

1. Drag an `.exe` installer into OpenNTX AppPortal.
2. OpenNTX analyzes the PE file.
3. OpenNTX creates an install/capture plan.
4. OpenNTX captures installer output into per-app state.
5. OpenNTX generates an app manifest.
6. OpenNTX creates a Linux desktop launcher.
7. The app appears in the Linux application menu.

## Current Status

OpenNTX V0.2 is an experimental foundation and PE analysis prototype.

It includes:

- repository structure
- architecture documentation
- JSON schemas
- example manifests and capture reports
- Rust core models
- real PE metadata analyzer for headers, sections, entry point, image base, subsystem, and imported DLL names
- CLI skeleton
- AppPortal TUI/mock skeleton
- development scripts
- basic tests

It does not run arbitrary Windows software. Runtime execution, installer capture, Win32 compatibility, DirectX translation, driver support, and full sandbox enforcement are future research areas.

## Architecture Overview

```text
Windows PE/EXE
    |
    v
PE analyzer
    |
    v
OpenNTX Core
    |
    +--> manifest resolver
    +--> sandbox policy
    +--> registry/filesystem overlay model
    +--> desktop integration
    +--> packaging layout
    |
    v
Runtime backend abstraction
    |
    +--> NotImplementedBackend        (V0.2)
    +--> ExternalCompatibilityBackend (future placeholder)
    +--> FutureNativeBackend          (future PE/NT/Win32 research)
```

The CLI and AppPortal call the core. Runtime logic must not live in the GUI.

## AppPortal Concept

AppPortal is the user-facing install surface. V0.2 ships as a lightweight TUI/mock that documents the intended flow without adding heavy GUI dependencies.

Planned AppPortal surfaces:

- Home with a large "Drop a Windows .exe installer here" area.
- File picker / drop zone.
- Analysis result view.
- Install wizard.
- Per-app sandbox permission selection.
- App library with Run, Settings, Repair, Package, Remove.
- Settings for default sandbox, runtime backend, compatibility database, diagnostics.

## CLI Concept

The `openntx` CLI is the stable automation surface for the core:

```bash
openntx analyze ~/Downloads/setup.exe
openntx install ~/Downloads/setup.exe
openntx run example-app
openntx package example-app
openntx remove example-app
openntx doctor ~/Downloads/setup.exe
```

V0.2 commands produce validation and planning output. They do not execute Windows binaries.

## Development

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Build and test:

```bash
cargo build --workspace
cargo test --workspace
tools/dev-check.sh
```

Optional schema validation:

```bash
python3 -m pip install --user jsonschema
tools/dev-check.sh
```

## How to Test V0.2

Run the required local checks:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
tools/dev-check.sh
```

Run CLI and AppPortal smoke checks:

```bash
cargo run -p openntx-cli -- package example-app
tools/mock-install-flow.sh
cargo run -p openntx-appportal
```

The mock install flow creates a temporary minimal PE fixture and demonstrates analysis/install planning only. OpenNTX V0.2 does not execute Windows binaries or installers.

See [docs/testing.md](docs/testing.md) for the full test guide and V0.2 test boundaries.

## Roadmap Summary

- Phase 0: concept and repository foundation.
- Phase 1: real PE analyzer implemented; manifest generator remains future work.
- Phase 2: AppPortal drag-and-drop install flow mock.
- Phase 3: desktop integration and launcher generation.
- Phase 4: installer capture prototype.
- Phase 5: runtime backend abstraction.
- Phase 6: experimental Win32/NT compatibility research.
- Phase 7: compatibility database and profiles.
- Phase 8: sandboxed app-store-style UX.

See [ROADMAP.md](ROADMAP.md) for the full plan.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [CONTRIBUTOR-LICENSE-TERMS.md](CONTRIBUTOR-LICENSE-TERMS.md) before submitting patches.

Contributions must keep OpenNTX honest: do not claim compatibility that has not been implemented and tested.
