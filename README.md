<p align="center">
  <img src="screenshots/banner.png" alt="OpenNTX Banner" width="850">
</p>

<p align="center">
  <img src="screenshots/app_icon.png" alt="OpenNTX App Icon" width="128">
</p>

# OpenNTX

**Drop EXE. Run Native.**

<p align="center">
  <a href="https://github.com/openntx/openntx/actions/workflows/ci.yml"><img src="https://github.com/openntx/openntx/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/status-experimental-orange" alt="Status">
  <img src="https://img.shields.io/badge/version-0.9.0--alpha-blue" alt="Version">
  <img src="https://img.shields.io/badge/license-PolyForm%20Noncommercial%201.0.0-blue" alt="License">
  <img src="https://img.shields.io/badge/runtime-not%20implemented-lightgrey" alt="Runtime">
</p>

OpenNTX is an experimental Windows application subsystem for Linux.

It aims to make Windows PE/EXE applications feel like native Linux desktop apps by combining PE detection, app manifests, installer capture, sandboxing, desktop integration, and a future NT/Win32 compatibility runtime.

> **Important:** OpenNTX does **not** execute Windows binaries or installers yet. Current capabilities are analysis-only and prototype packaging.

---

## Current Capabilities

OpenNTX V0.9 can do the following today:

- **PE/EXE Analyzer** — reads DOS headers, PE signatures, COFF headers, sections, entry points, imported DLLs, and architecture.
- **Manifest Generator** — converts PE metadata into structured OpenNTX app manifests.
- **Local App Registry** — writes per-app directories with manifest, install plan, metadata, and state.
- **Desktop Launcher Writer** — generates `.desktop` files for registered apps.
- **Run-Plan Diagnostics** — loads manifests, prints app metadata, writes JSON logs, and can notify the desktop.
- **Capture Snapshot/Diff** — snapshots app directory state, computes filesystem diffs, and generates capture reports.
- **Debian .deb Package Builder** — builds root-owned `.deb` packages with correct file permissions from registered apps.
- **AppPortal TUI** — terminal UI for browsing registered apps, analyzing EXEs, managing launchers, capture, and packaging.

---

## Not Implemented Yet

OpenNTX does **not** currently provide:

- Windows binary execution
- Installer execution
- Win32/NT runtime
- DirectX translation
- Driver support
- Wine, Proton, Bottles, or Lutris integration
- Full sandbox enforcement

These are future research areas. See [ROADMAP.md](ROADMAP.md) for the full plan.

---

## Quick Demo

This is the real V0.9 analysis and packaging flow. No Windows binary is executed at any step.

```bash
# 1. Analyze a Windows PE/EXE file
openntx analyze ~/Downloads/setup.exe

# 2. Generate an OpenNTX manifest from PE metadata
openntx manifest generate ~/Downloads/setup.exe --json

# 3. Register the app locally (writes manifest, install plan, metadata)
openntx install ~/Downloads/setup.exe --write-plan

# 4. Create a Linux desktop launcher
openntx desktop create <app-id> --yes

# 5. Snapshot the app directory before a capture step
openntx capture snapshot-before <app-id>

# 6. (Optional) Manually modify files in the app directory to simulate changes

# 7. Snapshot after the change
openntx capture snapshot-after <app-id>

# 8. Compute the filesystem diff
openntx capture diff <app-id>

# 9. Build a .deb package (dry-run by default, pass --yes to build)
openntx package build <app-id> --yes
```

Replace `<app-id>` with the registered app ID shown by `openntx list`.

**OpenNTX does not execute Windows binaries.** Every step above is analysis, metadata, or packaging only.

---

## CLI Examples

Use `<path-to-exe>` for a Windows PE/EXE path and `<app-id>` for a registered OpenNTX app ID.

### Analyze and Generate Metadata

```bash
openntx analyze <path-to-exe>
openntx manifest generate <path-to-exe>
openntx manifest generate <path-to-exe> --json
openntx manifest generate <path-to-exe> --output /tmp/app.openntx.json
```

### Write an Analysis-Only Install Plan

```bash
openntx install <path-to-exe>
openntx install <path-to-exe> --write-plan
openntx install <path-to-exe> --write-plan --desktop
```

### Manage Registered Apps

```bash
openntx list
openntx show <app-id>
openntx remove <app-id> --dry-run
openntx remove <app-id> --yes
```

### Desktop Launchers

```bash
openntx desktop create <app-id> --dry-run
openntx desktop create <app-id> --yes
openntx desktop remove <app-id> --dry-run
openntx desktop remove <app-id> --yes
```

### Run Plan (No Windows Execution)

```bash
openntx run <app-id>
openntx run <app-id> --json
openntx run <app-id> --notify
```

### Capture Snapshot/Diff

```bash
openntx capture snapshot-before <app-id>
openntx capture snapshot-after <app-id>
openntx capture diff <app-id>
openntx capture report <app-id>
openntx capture status <app-id>
openntx capture snapshot-before <app-id> --json
openntx capture diff <app-id> --json
```

### Build a .deb Package

```bash
openntx package build <app-id>              # dry-run (default)
openntx package build <app-id> --yes        # actually build
openntx package build <app-id> --output dist --version 1.0.0
```

### Diagnostics

```bash
openntx doctor <path-to-exe>
openntx doctor <app-id>
```

---

## AppPortal

AppPortal is the user-facing terminal UI.

<p align="center">
  <img src="screenshots/appportal_mockup.png" alt="OpenNTX AppPortal Terminal Mockup" width="850">
</p>

Current AppPortal surfaces:

- **Home** — version, registered app count, runtime status, action menu.
- **App Library** — lists registered apps from `~/.local/share/openntx/apps/`.
- **App Details** — manifest path, executable path, sandbox profile, DLL count, desktop launcher status, capture actions.
- **Analyze EXE** — reads PE metadata and previews generated manifests.
- **Install Plan** — writes registry metadata only after confirmation.
- **Desktop Launcher** — create/remove with confirmation.
- **Capture** — Snapshot Before/After, Diff, Report, Status per registered app.
- **Settings** — default sandbox, runtime backend, diagnostics, path layout.

---

## Architecture Overview

<p align="center">
  <img src="screenshots/architecture.png" alt="OpenNTX Architecture Overview" width="850">
</p>

```text
Windows PE/EXE
    |
    v
PE analyzer
    |
    v
OpenNTX Core
    |
    +--> manifest generator/resolver
    +--> sandbox policy
    +--> registry/filesystem overlay model
    +--> desktop integration
    +--> packaging layout
    |
    v
Runtime backend abstraction
    |
    +--> NotImplementedBackend        (current)
    +--> ExternalCompatibilityBackend (future placeholder)
    +--> FutureNativeBackend          (future PE/NT/Win32 research)
```

The CLI and AppPortal call the core. Runtime logic must not live in the GUI.

---

## Security Model

- OpenNTX does not execute Windows binaries or installers.
- PE analysis reads headers and metadata only — no code execution.
- App registry writes are local and isolated per app.
- Capture snapshots inspect OpenNTX app directories only.
- The `.deb` package builder uses `symlink_metadata()` to reject unsafe entries, sets 0644/0755 permissions, and builds with `--root-owner-group`.
- The sandbox model is documented in [docs/sandbox-model.md](docs/sandbox-model.md).

---

## Roadmap

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Concept and repository foundation | Done |
| 1 | PE analyzer and manifest generator | Done (V0.2/V0.3) |
| 2 | AppPortal registry UI | Done (V0.6) |
| 3 | Desktop integration and launcher generation | Done (V0.5) |
| 4 | Installer capture prototype | Partial (V0.8 snapshot/diff) |
| 5 | Runtime backend abstraction | Placeholder only |
| 6 | Win32/NT compatibility research | Not started |
| 7 | Compatibility database and profiles | Not started |
| 8 | Sandboxed app-store UX | Not started |
| V1.0-alpha | Polish, CI, visual assets, demo docs | Planned |

See [ROADMAP.md](ROADMAP.md) for the full plan.

---

## License and Commercial Use

OpenNTX is a source-available project.

The default license is **noncommercial**. Commercial use requires explicit written permission from Đặng Nhất Phi.

- [LICENSE](LICENSE) — PolyForm Noncommercial 1.0.0
- [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md) — commercial licensing terms
- [CONTRIBUTOR-LICENSE-TERMS.md](CONTRIBUTOR-LICENSE-TERMS.md) — contributor agreement

---

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [CONTRIBUTOR-LICENSE-TERMS.md](CONTRIBUTOR-LICENSE-TERMS.md) before submitting patches.

Contributions must keep OpenNTX honest: do not claim compatibility that has not been implemented and tested.

### Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Build and test
cargo build --workspace
cargo test --workspace
tools/dev-check.sh

# Install the CLI
cargo install --path crates/openntx-cli
```

### Running Checks

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
bash tools/dev-check.sh
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [docs/vision.md](docs/vision.md) | Project vision and long-term goals |
| [docs/architecture.md](docs/architecture.md) | System architecture |
| [docs/manifest-spec.md](docs/manifest-spec.md) | Manifest schema specification |
| [docs/sandbox-model.md](docs/sandbox-model.md) | Sandbox and permission model |
| [docs/runtime-design.md](docs/runtime-design.md) | Runtime backend design |
| [docs/installer-capture.md](docs/installer-capture.md) | Capture snapshot/diff design |
| [docs/packaging.md](docs/packaging.md) | Debian packaging design |
| [docs/desktop-integration.md](docs/desktop-integration.md) | Desktop launcher design |
| [docs/testing.md](docs/testing.md) | Testing guide |
| [docs/demo.md](docs/demo.md) | Step-by-step demo guide |
| [RELEASE_NOTES.md](RELEASE_NOTES.md) | Release notes for current version |
| [ROADMAP.md](ROADMAP.md) | Full development roadmap |
| [CHANGELOG.md](CHANGELOG.md) | Version changelog |
