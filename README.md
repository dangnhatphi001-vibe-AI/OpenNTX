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
  <img src="https://img.shields.io/badge/status-v1.4.0--alpha-orange" alt="Status">
  <img src="https://img.shields.io/badge/license-PolyForm%20Noncommercial%201.0.0-blue" alt="License">
  <img src="https://img.shields.io/badge/runtime-not%20implemented-lightgrey" alt="Runtime">
</p>

OpenNTX is an experimental **Windows Application Subsystem for Linux**.
It makes Windows PE/EXE applications feel like native Linux desktop apps by
combining PE detection, app manifests, compatibility profiles, real-time
installer capture, sandboxing, .deb packaging, and desktop integration — all
driven by a structured manifest and profile database.

> **Current status:** V1.4.0-alpha — analysis, profiling, real-time capture,
> and native .deb packaging are implemented. Runtime execution is **not**
> implemented yet.

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
compatibility profile, captures installer behaviour in real time, maps the
filesystem and registry, applies a sandbox policy, and packages everything
into a native `.deb` with a Linux desktop launcher. The application appears
in the system menu like any other installed program.

No prefixes. No wrapper scripts. No manual command lines.

---

## Roadmap — V1.x Series

- [x] **V1.0-alpha — Foundation**
  PE/EXE Analyzer, Manifest Generator, App Registry, Desktop Launcher,
  Run-Plan Diagnostics, Capture Snapshot/Diff, .deb Package Builder,
  App Management (rename, duplicate, export/import), Doctor/Integrity
  Checks, Logs, Config System, Shell Completions.

- [x] **V1.1.0 — AppPortal UX & Async Event Loop**
  Async TUI with crossterm event system (dedicated OS thread, no tokio
  blocking), Library/Details/Capture/Package/Doctor/Logs screens,
  background worker tasks, graceful terminal teardown with panic hook.

- [x] **V1.2.0 — Compatibility Profile Database**
  Per-application `CompatProfile` schema (metadata, runtime requirements,
  filesystem rules, registry rules, installer behaviour), `ProfileManager`
  with JSON persistence at `~/.local/share/openntx/profiles/`, integration
  into AppPortal TUI state.

- [x] **V1.3.0 — Advanced Installer Capture Workflow**
  Real-time filesystem capture engine using Linux `inotify`. Replaces the
  old snapshot-before/after + diff mechanism with zero-noise streaming
  events. See [V1.3.0 details](#v130--real-time-capture-engine) below.

- [x] **V1.4.0 — Debian Package Builder & Linux Integration**
  Profile-driven `.deb` package builder with automatic `DEBIAN/control`
  generation, native `.desktop` launcher, cross-filesystem safety, and
  `dpkg-deb` toolchain integration. See
  [V1.4.0 details](#v140--debian-package-builder) below.

---

## V1.3.0 — Real-time Capture Engine

The V1.3 capture system replaces the old snapshot-before/after + diff
mechanism (which required two full directory scans and produced noisy,
diff-based output) with a **streaming event model** built on Linux `inotify`.

### Architecture

```text
  ┌──────────────────────┐   inotify (non-blocking)   ┌─────────────────┐
  │  OS thread            │ ─────────────────────────► │  mpsc::Receiver  │
  │  inotify fd polling   │   CaptureEvent stream      │  caller thread   │
  └──────────────────────┘                             └─────────────────┘
```

### How it works

1. **Dicated OS thread** — `CaptureSession::start_tracking()` spawns a
   dedicated OS thread (via `std::thread::Builder`) that owns the inotify
   instance. This thread never touches the tokio async executor.

2. **Non-blocking polling** — The inotify file descriptor is set to
   `O_NONBLOCK` via `fcntl(F_SETFL)` immediately after `Inotify::init()`.
   The thread calls `read_events()` in a tight loop; when no events are
   queued, it receives `WouldBlock` and sleeps for 250 ms before retrying.
   This sleep window also serves as the shutdown check: when the
   `mpsc::Receiver` is dropped, the next `tx.send()` fails and the thread
   exits cleanly.

3. **Recursive auto-watch** — On startup, every subdirectory under the
   target directory is registered with `inotify.add_watch()`. When a
   `CREATE` + `ISDIR` event arrives (a new directory was created), the
   tracker immediately adds a watch for it — so files created inside new
   directories are captured without manual intervention.

4. **Event filtering** — Only `CREATE`, `MODIFY`, and `DELETE` events are
   forwarded. `ACCESS`, `OPEN`, `CLOSE_WRITE`, `ATTRIB`, and other
   read-only metadata events are silently ignored.

5. **Symlink rejection** — Consistent with the existing security model in
   `snapshot.rs`: every directory is verified with `symlink_metadata()`
   before watching, and the `DONT_FOLLOW` watch flag prevents the kernel
   from following symlinks.

### Events

```rust
pub enum CaptureEvent {
    FileCreated(PathBuf),   // A file or directory was created
    FileModified(PathBuf),  // A file was modified
    FileDeleted(PathBuf),   // A file or directory was deleted
}
```

### Test coverage

9 tests covering: file creation, modification, deletion, nested directory
events, file-in-new-subdirectory tracking, event ordering, receiver drop
shutdown, and error paths.

---

## V1.4.0 — Debian Package Builder

The V1.4 package builder takes a `CompatProfile` (from the V1.2 profile
database) and produces a standards-compliant `.deb` package that installs
the Windows application as a native Linux desktop application.

### Build lifecycle

```text
  CompatProfile
       │
       ▼
  DebBuilder::new(profile)
       │
       ├── prepare_workspace()        ← /tmp/openntx_builder_<app_id>/
       │     DEBIAN/                      control file goes here
       │     opt/openntx/apps/<id>/       app files go here
       │     usr/share/applications/      .desktop launcher goes here
       │
       ├── generate_control_file()    ← DEBIAN/control
       │     Package, Version, Architecture (amd64/i386),
       │     Depends: openntx-cli, Maintainer, Description
       │
       ├── generate_desktop_entry()   ← .desktop launcher
       │     Exec=openntx run <app_id>
       │     X-OpenNTX-AppId, X-OpenNTX-Publisher
       │
       └── build_deb()                ← dpkg-deb --root-owner-group --build
             Cross-filesystem rename fallback (copy+remove)
             Auto-cleanup staging on success
             Preserved on failure for debugging
```

### Key design decisions

- **Profile-driven** — Architecture, version, name, and publisher all come
  from the `CompatProfile`. No manual configuration needed.
- **Native desktop integration** — The `.desktop` file is installed to
  `usr/share/applications/`, so the Windows application appears in the
  system application menu alongside native Linux apps.
- **Cross-filesystem safety** — The `.deb` is built in the system temp
  directory (so Unix permission normalisation works on any filesystem),
  then moved to the output directory with a `rename()` → `copy()+remove()`
  fallback for cross-filesystem moves.
- **Staging auto-cleanup** — The staging workspace is removed on success
  and preserved on failure, so the user can inspect the build tree.

### Test coverage

10 tests covering: workspace structure, idempotency, control file content,
architecture mapping (x86 → i386, x86_64 → amd64), desktop entry content,
missing control file error, filename derivation, builder options, and
dpkg-deb availability.

---

## Tech Stack

**Language:** Rust (edition 2021, MSRV 1.75)

**Cargo Workspace:**

```text
crates/
  openntx-core       Core engine — data models, validation, path layout,
                     PE analysis, manifest generation, compatibility
                     profiles, real-time capture (inotify), package
                     building (.deb), runtime planning, desktop
                     integration, doctor, logs, config, sandbox.

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
| `inotify` | Linux filesystem event monitoring (real-time capture) |
| `libc` | Low-level POSIX calls (`fcntl`, `O_NONBLOCK`) |
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

## Towards V2.x — Execution & Integration

The V1.x series built the **identity and packaging layer**: every Windows
application now has a manifest, a compatibility profile, captured filesystem
and registry behaviour, and a native `.deb` package with a desktop launcher.

V2.x will add the **execution layer**:

- **`binfmt_misc` integration** — Register the OpenNTX runtime as a Linux
  binary format handler so that `.exe` files are transparently executed
  through the OpenNTX subsystem when double-clicked or invoked from the
  shell.
- **OpenNTX Runtime** — A sandboxed execution environment that reads the
  compatibility profile at launch time, sets up the filesystem overlay,
  applies registry mappings, enforces the sandbox policy, and runs the
  Windows application inside an isolated Wine-compatible (but
  Wine-independent) container.
- **Runtime backend abstraction** — The `NotImplementedBackend` placeholder
  will be replaced with real backends: a Wine-based compatibility backend
  for broad application support, and a future native PE/NT/Win32 backend
  for research.

The compatibility profile database, the sandbox model, and the
manifest-driven architecture exist specifically so that OpenNTX can evolve
its own runtime without depending on external compatibility layers.

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

# Register an app and load its profile
./target/release/openntx register /path/to/app.exe

# Launch the TUI
./target/release/openntx-appportal

# Build a .deb package
./target/release/openntx package build <app-id> --yes

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
| [docs/installer-capture.md](docs/installer-capture.md) | Installer capture workflow |
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
