<p align="center">
  <img src="screenshots/banner.png" alt="OpenNTX Banner" width="850">
</p>

<p align="center">
  <img src="screenshots/app_icon.png" alt="OpenNTX App Icon" width="128">
</p>

# OpenNTX

**Windows Application Subsystem for Linux**

<p align="center">
  <a href="https://github.com/openntx/openntx/actions/workflows/ci.yml"><img src="https://github.com/openntx/openntx/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/status-v1.2.0--alpha-orange" alt="Status">
  <img src="https://img.shields.io/badge/license-PolyForm%20Noncommercial%201.0.0-blue" alt="License">
  <img src="https://img.shields.io/badge/runtime-not%20implemented-lightgrey" alt="Runtime">
</p>

OpenNTX is an experimental **Windows Application Subsystem for Linux**.
It makes Windows PE/EXE applications feel like native Linux desktop apps by
combining PE detection, app manifests, compatibility profiles, installer
capture, sandboxing, and desktop integration — all driven by a structured
manifest and profile database.

> **Current status:** V1.2.0-alpha — analysis, profiling, and packaging are
> implemented. Runtime execution is **not** implemented yet.

---

## Core Vision & Philosophy

OpenNTX is **not** a Wine frontend. It is **not** Proton. It is **not** a
prefix manager.

OpenNTX is a **subsystem** — a structured layer that sits between a Windows
application and the Linux host, translating application identity, filesystem
layout, registry expectations, and runtime requirements into native Linux
desktop semantics.

The execution chain:

```text
  Windows Application (.exe / .msi)
           │
           ▼
  ┌─────────────────────────┐
  │   OpenNTX Runtime        │  PE analysis · Manifest resolution
  │   (identity & planning)  │  Compatibility profile lookup
  └────────────┬────────────┘
               │
               ▼
  ┌─────────────────────────┐
  │   OpenNTX Services       │  Sandbox policy · Registry overlay
  │   (isolation & mapping)  │  Filesystem mapping · Desktop integration
  └────────────┬────────────┘
               │
               ▼
  ┌─────────────────────────┐
  │   Linux Host             │  Native launcher · Isolated state
  │   (desktop & process)    │  Logs · Uninstall metadata
  └─────────────────────────┘
```

The user drops an EXE. OpenNTX analyses it, generates a manifest, resolves a
compatibility profile, maps the filesystem and registry, applies a sandbox
policy, and produces a native Linux desktop launcher. The application runs in
an isolated environment with structured logs and clear permission boundaries.

No prefixes. No wrapper scripts. No manual command lines.

---

## Roadmap — V1.x Series

- [x] **V1.0-alpha — Foundation**
  PE/EXE Analyzer, Manifest Generator, App Registry, Desktop Launcher,
  Run-Plan Diagnostics, Capture Snapshot/Diff, .deb Package Builder,
  App Management (rename, duplicate, export/import), Doctor/Integrity
  Checks, Logs, Config System, Shell Completions.

- [x] **V1.1.0 — AppPortal UX & Async Demo Flow**
  Async TUI with crossterm event system (dedicated OS thread, no tokio
  blocking), Library/Details/Capture/Package/Doctor/Logs screens,
  background worker tasks, graceful terminal teardown with panic hook.

- [x] **V1.2.0 — Compatibility Profile Database**
  Per-application `CompatProfile` schema (metadata, runtime requirements,
  filesystem rules, registry rules, installer behaviour), `ProfileManager`
  with JSON persistence at `~/.local/share/openntx/profiles/`, integration
  into AppPortal TUI state.

- [ ] **V1.3.0 — Advanced Installer Capture Workflow**
  Guided multi-step capture: pre-install snapshot, installer execution
  sandbox, post-install snapshot, automated diff analysis, profile
  auto-generation from capture data.

---

## Tech Stack

**Language:** Rust (edition 2021, MSRV 1.75)

**Cargo Workspace:**

```text
crates/
  openntx-core       Core engine — data models, validation, path layout,
                     PE analysis, manifest generation, compatibility
                     profiles, runtime planning, desktop integration,
                     capture, packaging, doctor, logs, config, sandbox.

  openntx-cli        Command-line interface for automation, diagnostics,
                     and scripted workflows.  JSON output for every
                     command.  Shell completions (bash, zsh, fish).

  openntx-appportal  Async TUI frontend built with ratatui + crossterm.
                     Dedicated OS thread for input polling.  Background
                     worker tasks via tokio::task::spawn_blocking.
```

**Key dependencies:**

| Crate | Purpose |
|---|---|
| `ratatui` | Terminal UI rendering |
| `crossterm` | Terminal input/output (raw mode, alternate screen) |
| `tokio` | Async runtime for background tasks |
| `serde` / `serde_json` | JSON serialization for manifests, profiles, configs |
| `thiserror` | Structured error types |
| `sha2` | SHA-256 hashing for integrity checks |
| `tar` / `flate2` | Export/import bundle compression |
| `dirs` | XDG-compliant data directory resolution |
| `anyhow` | Error propagation in CLI |

**Data formats:**

| Schema | Location |
|---|---|
| App Manifest | `schemas/app-manifest.schema.json` |
| Compatibility Profile | `schemas/compatibility-profile.schema.json` |
| Capture Report | `schemas/capture-report.schema.json` |
| Capture Snapshot | `schemas/capture-snapshot.schema.json` |
| Capture Diff | `schemas/capture-diff.schema.json` |

---

## The Golden Rule

> **OpenNTX will never become a Wine manager.**

Wine is a compatibility layer that translates Windows API calls in real time.
Proton is a Wine distribution optimised for gaming. Lutris, Bottles, and
PlayOnLinux are prefix managers that wrap Wine with configuration UIs.

OpenNTX is none of these.

OpenNTX builds its own **application identity layer** — manifests, profiles,
sandbox policies, filesystem mappings, registry overlays — so that a Windows
application can be managed, isolated, and integrated into the Linux desktop
as a structured, auditable entity.

When a runtime backend is implemented, it will be an **OpenNTX service** —
not a wrapper around Wine. The compatibility profile database, the sandbox
model, and the manifest-driven architecture exist specifically so that
OpenNTX can evolve its own runtime without depending on external compatibility
layers.

---

## Quick Start

```bash
# Build the workspace
cargo build --release

# Analyse a Windows EXE
./target/release/openntx analyze /path/to/app.exe

# Register an app
./target/release/openntx register /path/to/app.exe

# Launch the TUI
./target/release/openntx-appportal

# Run doctor diagnostics
./target/release/openntx doctor --global
```

---

## Documentation

| Document | Description |
|---|---|
| [docs/vision.md](docs/vision.md) | Long-term vision and design philosophy |
| [docs/architecture.md](docs/architecture.md) | System architecture and module boundaries |
| [docs/manifest-spec.md](docs/manifest-spec.md) | App manifest specification |
| [docs/sandbox-model.md](docs/sandbox-model.md) | Sandbox and isolation model |
| [docs/runtime-design.md](docs/runtime-design.md) | Runtime backend design |
| [docs/capture-snapshot.md](docs/installer-capture.md) | Installer capture workflow |
| [docs/packaging.md](docs/packaging.md) | .deb package builder |
| [docs/desktop-integration.md](docs/desktop-integration.md) | Desktop launcher generation |
| [docs/testing.md](docs/testing.md) | Test strategy and coverage |
| [ROADMAP.md](ROADMAP.md) | Detailed roadmap with milestones |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guidelines |

---

## License

OpenNTX is licensed under the
[PolyForm Noncommercial License 1.0.0](LICENSE).
See [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md) for commercial use.
