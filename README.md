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
  <img src="https://img.shields.io/badge/version-v3.0.0--alpha-brightgreen" alt="Version">
  <img src="https://img.shields.io/badge/era-Consumer%20Edition-critical" alt="Era">
  <img src="https://img.shields.io/badge/license-PolyForm%20Noncommercial%201.0.0-blue" alt="License">
  <img src="https://img.shields.io/badge/runtime-LIVE-brightgreen" alt="Runtime">
</p>

> **Version: v3.0.0-alpha — The Consumer Edition**
>
> OpenNTX is now a **1-click install** application. `sudo dpkg -i openntx.deb`
> and the entire platform is live — systemd daemon auto-starts, `.exe` files
> are intercepted by the kernel via `binfmt_misc`, the Slint GUI launches
> from the application menu, and the API server runs headless in the background.
>
> The runtime executes Windows applications in isolated sandboxes with cgroups v2
> resource governance and namespace isolation. The reaper engine culls zombie
> processes. The Slint-based graphical frontend provides drag-and-drop file
> deployment, native file dialogs, and real-time system monitoring. The
> AppPortal daemon auto-detects headless environments (systemd) and parks
> the API server with `std::future::pending()` — no TUI, no crash loops.
>
> This is no longer a subsystem under construction — **it is a consumer product.**

---

## What OpenNTX Actually Is

OpenNTX is **not** a Wine frontend. It is **not** Proton. It is **not** a
prefix manager.

OpenNTX is a **subsystem** — a structured layer that sits between a Windows
application and the Linux host. It translates application identity, filesystem
layout, registry expectations, and runtime requirements into native Linux
desktop semantics.

When you double-click a `.exe` on an OpenNTX-enabled system:

1. The **Linux kernel** intercepts the execution via `binfmt_misc`.
2. The kernel redirects to `/usr/bin/openntx-runtime`.
3. The runtime **SHA-256 hashes** the PE file and queries the Profile Database.
4. If a profile exists → **isolated execution** with full sandbox policy.
5. If no profile exists → **Intelligent Auto-Fallback**: the runtime silently
   activates a `CaptureSession`, executes the PE, records every filesystem
   change, and **auto-generates a compatibility profile** — all on the first run.
6. Throughout the capture, **live status** is streamed via Unix Domain Socket
   to the TUI AppPortal, which displays a blinking `⚠ [KERNEL] SYSTEM IS
   CAPTURING` banner in real time.

No prefixes. No wrapper scripts. No manual configuration. The subsystem
learns.

---

## Data Flow — Kernel to Glass

```text
  ┌─────────────┐
  │  User runs   │   $ ./app.exe   or   double-click in file manager
  │  a .exe file │
  └──────┬──────┘
         │
         ▼
  ┌──────────────────────────────────────────────────────────────┐
  │  Linux Kernel — binfmt_misc                                   │
  │  Detects MZ header -> traps execution                        │
  │  Redirects to: /usr/bin/openntx-runtime                      │
  └──────┬───────────────────────────────────────────────────────┘
         │
         ▼
  ┌──────────────────────────────────────────────────────────────┐
  │  RuntimeEntrypoint::dispatch_execution()                      │
  │                                                               │
  │  ┌─────────────┐    ┌─────────────────┐    ┌──────────────┐ │
  │  │ SHA-256 Hash │-->│ ProfileManager   │-->│ Profile      │ │
  │  │ of PE file   │    │ lookup (V1.2)    │    │ exists?      │ │
  │  └─────────────┘    └─────────────────┘    └──────┬───────┘ │
  │                                                    │         │
  │                          ┌─────────────────────────┤         │
  │                          │                         │         │
  │                     YES v                    NO v            │
  │              ┌──────────────┐     ┌───────────────────────┐ │
  │              │ OpenNTXExec  │     │ Auto-Fallback Capture │ │
  │              │ .execute_pe()│     │                       │ │
  │              │ (direct run) │     │ 1. Create sandbox     │ │
  │              └──────────────┘     │ 2. CaptureSession     │ │
  │                                   │ 3. Execute PE (1st)   │ │
  │                                   │ 4. Record events      │ │
  │                                   │ 5. Build CompatProfile│ │
  │                                   │ 6. Save to DB         │ │
  │                                   └───────────┬───────────┘ │
  │                                               │             │
  └───────────────────────────────────────────────┘             │
                                                                │
  ┌─────────────────────────────────────────────────────────────┘
  │
  │   During capture (V2.2 IPC Bridge):
  │
  │   ┌────────────────┐    Unix Domain     ┌──────────────────┐
  │   │ RuntimeIpcClient│---Socket (UDS)---->│ RuntimeIpcServer │
  │   │ sends JSON lines│   /tmp/openntx_    │ receives & fwd   │
  │   │ per event       │   runtime.sock     │ via mpsc channel │
  │   └────────────────┘                    └────────┬─────────┘
  │                                                   │
  │                                                   v
  │                                        ┌──────────────────┐
  │                                        │ TUI AppPortal     │
  │                                        │                   │
  │                                        │ ⚠ [KERNEL]        │
  │                                        │ SYSTEM IS         │
  │                                        │ CAPTURING:        │
  │                                        │ app-abc123 -- 47  │
  │                                        │ files tracked     │
  │                                        └──────────────────┘
  v
  ┌──────────────────────────────────────────────────────────────┐
  │  Linux Host                                                    │
  │  Isolated Wine prefix · Native .desktop launcher · Logs       │
  └──────────────────────────────────────────────────────────────┘
```

---

## Roadmap — The Complete Journey

- [x] **V1.0-alpha — Foundation**
  PE/EXE Analyzer, Manifest Generator, App Registry, Desktop Launcher,
  Run-Plan Diagnostics, Capture Snapshot/Diff, Doctor, Logs, Config,
  Shell Completions.

- [x] **V1.1.0 — AppPortal UX & Async Event Loop**
  Async TUI with crossterm (dedicated OS thread), Library/Details/Capture/
  Package/Doctor/Logs screens, background workers, graceful terminal teardown.

- [x] **V1.2.0 — Compatibility Profile Database**
  Per-app `CompatProfile` schema (metadata, runtime reqs, filesystem rules,
  registry rules, installer behaviour), `ProfileManager` with JSON persistence
  at `~/.local/share/openntx/profiles/`.

- [x] **V1.3.0 — Advanced Installer Capture Workflow**
  Real-time filesystem capture via Linux `inotify`. Streaming event model
  replaces old snapshot-before/after. Recursive auto-watch, symlink rejection,
  non-blocking polling with 250ms idle sleep.

- [x] **V1.4.0 — Debian Package Builder & Linux Integration**
  Profile-driven `.deb` builder with `DEBIAN/control` generation, native
  `.desktop` launcher, cross-filesystem safety, `dpkg-deb` toolchain.

- [x] **V2.0.0 — Kernel Integration via binfmt_misc & Isolated PE Executor**
  `BinfmtManager` registers PE format (`MZ` magic) with Linux kernel.
  `OpenNTXExecutor` orchestrates SHA-256 identification -> profile lookup ->
  sandbox setup -> Wine headless execution with `WINEDEBUG=-all` isolation.

- [x] **V2.1.0 — Runtime Entrypoint with SHA256 Profiling & Auto-Fallback**
  `RuntimeEntrypoint` parses kernel-supplied arguments, dispatches execution.
  **Intelligent Auto-Fallback**: on first run of an unknown PE, automatically
  activates `CaptureSession`, executes the PE, analyses captured events,
  and persists a new `CompatProfile` — zero user intervention.

- [x] **V2.2.0 — Live Monitoring & TUI Integration via UDS IPC**
  Unix Domain Socket IPC bridge between runtime process and TUI process.
  `RuntimeIpcServer` listens on `/tmp/openntx_runtime.sock`.
  `RuntimeIpcClient` sends JSON-line status updates during capture.
  TUI displays real-time blinking banner: `⚠ [KERNEL] SYSTEM IS CAPTURING`.

- [x] **V2.3.0 — cgroups v2 Resource Governance & Namespace Hardening**
  `ResourceGovernor` creates per-app cgroup subtrees for memory and CPU
  limits. `SecurityConfig` tiers (hardened/permissive/default) control
  `CLONE_NEWNET` and `CLONE_NEWNS` namespace isolation via `pre_exec`.

- [x] **V2.4.0 — Process Reaper Engine**
  `ReaperEngine` reads PIDs from `cgroup.procs`, sends SIGTERM, waits
  500ms grace period, then SIGKILLs survivors. `cleanup_cgroup_node`
  removes the cgroup directory after all processes are dead.

- [x] **V2.5.0 — Graphics Stub & Virtual Window Mapping**
  `WindowStubManager` allocates virtual HWNDs for sandboxed Windows
  applications. Headless mode: memory-mapped RGBA framebuffer files.
  X11 mode: display connection intent recorded. `map_gdi_flush` receives
  raw pixel data from the Windows emulation layer.

- [x] **V2.6.0 — Isolated Registry Emulation**
  `VirtualRegistry` emulates Windows Registry hive structure (HKLM, HKCU,
  etc.) using per-app TOML files in the sandbox. `get_value`/`set_value`
  with immediate disk flush. Supports nested key paths, value listing,
  deletion, and TOML persistence across process restarts.

- [x] **V2.6.5 — RESTful API Bridge for GUI Integration**
  `ApiBridgeServer` (axum) exposes `POST /api/v1/execute` and
  `POST /api/v1/purge` endpoints. Bridges GUI AppPortal to the runtime
  kernel. Supports hardened/permissive security modes.

- [x] **V2.7.0 — Slint GUI Frontend & API Client Bridge**
  `openntx-gui` crate with Slint UI (Cyberpunk/Industrial Dark Mode).
  System monitor header (PIDs, RAM, CPU), app grid view, deploy button,
  real-time log terminal. Async API client via `reqwest` with 1s polling
  loop for live monitor data.

- [x] **V2.8.0 — Automated Debian Package Generation Engine**
  Full platform `.deb` packaging via `openntx system-package build`.
  Generates `DEBIAN/control`, `postinst`, `prerm`. Default `sandbox.toml`.
  Shell automation via `tools/build-deb.sh`.

- [x] **V2.8.1 — Native File Dialog & Drag-and-Drop GUI**
  `openntx-gui` upgraded with `rfd` (native XDG Desktop Portal file picker),
  `winit` drag-and-drop via `WinitWindowAccessor::on_winit_window_event`,
  central Drop Zone UI with Browse button, and DEPLOY EXECUTION TARGET button.

- [x] **V3.0.0 — Consumer Edition: 1-Click .deb, Systemd, Headless Daemon**
  `openntx-core.service` systemd unit auto-starts API server on boot.
  `postinst`: `systemctl enable/start openntx-core`. `prerm`: `systemctl
  disable/stop` + Reaper Engine cleanup. `Depends:` now requires `wine-binfmt
  | wine`, `cgroup-tools`, `systemd`. Desktop entry (`openntx.desktop`) with
  MIME type `application/x-ms-dos-executable`. AppPortal headless mode:
  `IsTerminal` check → `std::future::pending()` parks API server when no TTY.
  GUI auto-fallback launcher: spawns daemon if API server not detected.

---

## Core Subsystems — V2.x Execution Era

### Kernel Subsystem Execution (V2.0 & V2.1)

The `binfmt_misc` mechanism allows the Linux kernel to recognise custom
executable formats. OpenNTX registers the PE format by writing the magic
string `:OpenNTX:M::MZ::/usr/bin/openntx-runtime:OC` to
`/proc/sys/fs/binfmt_misc/register`.

From that moment, **every `.exe` file on the system** is intercepted by the
kernel at the `execve()` level. The kernel sees the `MZ` header, matches the
binfmt rule, and redirects execution to the OpenNTX runtime binary.

**The Entrypoint (`RuntimeEntrypoint`):**

```rust
// parse_kernel_args: argv[0] = interpreter, argv[1] = PE path, argv[2..] = args
let (exe_path, app_args) = RuntimeEntrypoint::parse_kernel_args(env_args)?;

// dispatch_execution: SHA-256 -> profile lookup -> execute or fallback
entrypoint.dispatch_execution(&exe_path, &app_args)?;
```

**Intelligent Auto-Fallback** — When a PE file has no existing profile:

1. An isolated sandbox directory is created under
   `~/.local/share/openntx/sandboxes/<app_id>/`.
2. A `CaptureSession` starts inotify tracking on the entire sandbox tree.
3. The PE is executed for the first time through Wine headless.
4. Every `FileCreated` and `FileModified` event is collected.
5. A `CompatProfile` is constructed from the captured paths (Windows-style
   `C:\...` required paths, deduplicated and normalised).
6. The profile is persisted to `~/.local/share/openntx/profiles/<app_id>.json`.
7. All subsequent runs use the profile for guided isolation.

The user never sees any of this. They double-click an `.exe` and it just works.

**The Executor (`OpenNTXExecutor`):**

- Validates PE magic bytes (`MZ` header check).
- Computes SHA-256 hash for deterministic app identification.
- Queries `ProfileManager` for existing `CompatProfile`.
- Prepares isolated Wine prefix with `drive_c` structure.
- Selects Wine binary based on architecture (x86 -> `wine`, x86_64 -> `wine64`).
- Sets `WINEPREFIX`, `WINEDEBUG=-all`, `WINEESYNC=1` for clean isolation.
- Suppresses all Wine stdout/stderr noise.

### Real-time TUI IPC Bridge (V2.2)

The runtime and the TUI are separate processes. They communicate through a
**Unix Domain Socket** at `/tmp/openntx_runtime.sock` using a JSON-line
protocol.

**Protocol:**

```json
{"app_id":"notepadpp-a3f2","status":"Capturing","files_tracked":47}
```

**Server side (`RuntimeIpcServer`):**

- Binds to `/tmp/openntx_runtime.sock` (removes stale socket on startup).
- Non-blocking accept loop on a dedicated OS thread (100ms poll interval).
- Reads newline-delimited JSON from each connection.
- Forwards `CaptureStatusMessage`s through a `mpsc::Receiver` channel.
- Auto-cleans socket file on `Drop`.

**Client side (`RuntimeIpcClient`):**

- Short-lived connections: connect -> write JSON line -> flush -> close.
- `try_send_status()` for fire-and-forget (silently ignores missing server).
- Integrated into `RuntimeEntrypoint::fallback_capture_and_execute()`:
  sends `Capturing` -> `Executing` -> `Capturing(N files)` -> `Complete`.

**TUI integration:**

- `RuntimeIpcServer` spawned at AppPortal startup.
- Forwarding thread converts IPC messages -> `AppEvent::IpcCaptureStatus`.
- Tokio event loop delivers to `AppState::handle_ipc_capture_status()`.
- Footer renders blinking banner with alternating Red/Yellow colors:

```text
  ⚠ [KERNEL] SYSTEM IS CAPTURING: Capturing: notepadpp-a3f2 — 47 files tracked
```

---

## V1.3.0 — Real-time Capture Engine (Reference)

The capture system uses Linux `inotify` with a dedicated OS thread.

```text
  ┌──────────────────────┐   inotify (non-blocking)   ┌─────────────────┐
  │  OS thread            │ ─────────────────────────► │  mpsc::Receiver  │
  │  inotify fd polling   │   CaptureEvent stream      │  caller thread   │
  └──────────────────────┘                             └─────────────────┘
```

- **Non-blocking polling** — `O_NONBLOCK` via `fcntl(F_SETFL)`. 250ms sleep
  between idle polls. Shutdown via receiver drop detection.
- **Recursive auto-watch** — New directories (`CREATE + ISDIR`) are
  automatically watched. Files created inside new subdirectories are captured.
- **Event filtering** — Only `CREATE`, `MODIFY`, `DELETE`. All other inotify
  events silently ignored.
- **Symlink rejection** — `symlink_metadata()` + `DONT_FOLLOW` flag.

---

## V1.4.0 — Debian Package Builder (Reference)

Profile-driven `.deb` builder:

```text
  CompatProfile -> DebBuilder -> prepare_workspace() -> generate_control_file()
                 -> generate_desktop_entry() -> build_deb() -> .deb output
```

- Architecture mapping: x86 -> `i386`, x86_64 -> `amd64`.
- Cross-filesystem safety: `rename()` -> `copy()+remove()` fallback.
- Staging auto-cleanup on success, preserved on failure.

---

## Tech Stack

**Language:** Rust (edition 2021, MSRV 1.75)

**Cargo Workspace:**

```text
crates/
  openntx-core       Core engine — PE analysis, manifests, profiles,
                     real-time capture (inotify), package building (.deb),
                     runtime subsystem (binfmt, executor, entrypoint, IPC),
                     desktop integration, doctor, logs, config, sandbox.

  openntx-cli        CLI for automation and diagnostics. JSON output.
                     Shell completions (bash, zsh, fish).

  openntx-appportal  Async TUI (ratatui + crossterm). Dedicated OS thread
                     for input. Tokio background workers. Live IPC bridge.
                     Headless mode: IsTerminal check → std::future::pending()
                     parks API server when no TTY (systemd compatible).

  openntx-gui        Slint-based graphical frontend (Cyberpunk Dark Mode).
                     Native file dialog (rfd), drag-and-drop (winit),
                     Drop Zone UI, deploy button. Auto-fallback launcher:
                     spawns daemon if API server not detected.

  packaging/         System-level .deb packaging engine (V3.0).
  (inside core)      build_system_deb() → staging layout → DEBIAN/control,
                     postinst (systemd enable/start), prerm (Reaper Engine).
                     Systemd service, desktop entry, sandbox.toml.
                     tools/build-deb.sh automation.
```

**Runtime module structure (`openntx-core/src/runtime/`):**

```text
  runtime/
    mod.rs           Module root and re-exports
    binfmt.rs        BinfmtManager — kernel binfmt_misc registration
    executor.rs      OpenNTXExecutor — PE validation, sandbox, Wine launch
    entrypoint.rs    RuntimeEntrypoint — kernel args, dispatch, auto-fallback
    ipc.rs           RuntimeIpcServer/Client — UDS live status bridge
    cgroups.rs       ResourceGovernor — cgroups v2 memory/CPU limits
    reaper.rs        ReaperEngine — SIGTERM/SIGKILL process reaping
    graphics.rs      WindowStubManager — virtual HWND & framebuffer
    registry.rs      VirtualRegistry — isolated Windows Registry emulation
    api.rs           ApiBridgeServer — RESTful API bridge (axum)
    backend.rs       RuntimeBackend trait and execution plans
    placeholder.rs   NotImplemented/External/Future backend stubs
    run_plan.rs      Run-plan generation and logging
```

**Key dependencies:**

| Crate | Purpose |
|---|---|
| `ratatui` | Terminal UI rendering |
| `crossterm` | Terminal I/O (raw mode, alternate screen) |
| `slint` | GUI framework (Cyberpunk Dark Mode) |
| `rfd` | Native file dialog (XDG Desktop Portal) |
| `winit` | Window management, drag-and-drop events |
| `tokio` | Async runtime for TUI event loop and workers |
| `reqwest` | HTTP client for API bridge communication |
| `axum` | RESTful API server (execute, purge endpoints) |
| `serde` / `serde_json` | JSON serialization (manifests, profiles, IPC) |
| `inotify` | Linux filesystem event monitoring |
| `libc` | POSIX calls (`fcntl`, `geteuid`, `O_NONBLOCK`) |
| `thiserror` | Structured error types with `thiserror::Error` |
| `sha2` | SHA-256 hashing for PE identification |
| `tar` / `flate2` | Export/import bundle compression |
| `dirs` | XDG data directory resolution |
| `tempfile` | Secure temporary directories (tests) |

**Data formats:**

| Schema | Location |
|---|---|
| App Manifest | `schemas/app-manifest.schema.json` |
| Compatibility Profile | `schemas/compatibility-profile.schema.json` |
| Capture Report | `schemas/capture-report.schema.json` |
| Capture Snapshot | `schemas/capture-snapshot.schema.json` |
| Capture Diff | `schemas/capture-diff.schema.json` |

---

## Test Coverage

**339 tests, 0 failures.** Full breakdown:

| Module | Tests | Coverage |
|---|---|---|
| `runtime::binfmt` | 12 | Registration string, MZ magic, root check, field count, custom path |
| `runtime::executor` | 12 | PE detection, SHA-256, sandbox creation, Wine env, args passthrough |
| `runtime::entrypoint` | 17 | Kernel args parsing, dispatch flow, fallback capture, profile build |
| `runtime::ipc` | 9 | Message round-trip, server-client E2E, cleanup, error handling |
| `runtime::cgroups` | 17 | Path generation, sanitization, cpu_max_from_percent, apply_limits, cleanup |
| `runtime::reaper` | 19 | PID parsing, SIGTERM/SIGKILL flow, cgroup cleanup, signal helpers |
| `packaging::system_deb` | 10 | Control file format, postinst/prerm scripts, TOML validation, deps, systemd service, desktop entry |
| `runtime::graphics` | 22 | Surface creation, HWND allocation, GDI flush, framebuffer I/O, destruction |
| `runtime::registry` | 24 | Write-then-read, persistence, TOML validation, hive CRUD, key normalization |
| `runtime::api` | 6 | Execute endpoint, purge endpoint, validation, error handling, 404 |
| `openntx-gui` | 11 | App ID derivation, log state, monitor response, execute response, timestamp, is_exe_file |
| `profile` | 12 | CRUD, round-trip, arch serde, optional fields |
| `capture::realtime` | 7 | inotify events, nested dirs, ordering, shutdown |
| `capture::snapshot/diff` | 14 | Snapshots, diffs, symlinks, registry tracking |
| `packaging::deb` | 10 | Control file, desktop entry, staging, dpkg-deb |
| `builder::debian` | 10 | Workspace, control, desktop, options |
| `pe_analyzer` | 6 | PE32/PE64 detection, imports, architecture |
| `manifest` | 10 | Generation, validation, JSON schema |
| `registry` | 9 | Registration, desktop, run-plan, remove |
| `v1_alpha` | 33 | Config, doctor, rename, duplicate, export/import, logs |
| `doctor` | 4 | Global/app diagnostics, repair |
| Integration tests | 59 | End-to-end workflows |
| Doc-tests | 7 | Compile-check all public doc examples |

---

## Quick Start

```bash
# Build the workspace
cargo build --release

# Analyse a Windows EXE
./target/release/openntx analyze /path/to/app.exe

# Register an app
./target/release/openntx register /path/to/app.exe

# Launch the TUI (IPC server starts automatically)
./target/release/openntx-appportal

# Build a .deb package (per-app)
./target/release/openntx package build <app-id> --yes

# Build the full system .deb (V3.0 Consumer Edition)
./tools/build-deb.sh --version 3.0.0
# Or via CLI:
./target/release/openntx system-package build --version 3.0.0

# Install and run (after dpkg -i)
sudo dpkg -i target/debian/openntx_3.0.0_amd64.deb
# Daemon auto-starts via systemd. GUI available in app menu.
# Or launch manually:
./target/release/openntx-gui

# Register binfmt_misc (requires root)
sudo ./target/release/openntx runtime register

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

## The Golden Rule

> **OpenNTX will never become a Wine manager.**

Wine is a compatibility layer. Proton is a Wine distribution. Lutris, Bottles,
and PlayOnLinux are prefix managers.

OpenNTX is a **subsystem**.

It builds its own application identity layer — manifests, profiles, sandbox
policies, filesystem mappings, registry overlays, kernel-level PE interception,
and intelligent auto-capture — so that a Windows application can be managed,
isolated, and integrated into the Linux desktop as a structured, auditable
entity.

The runtime uses Wine as an execution backend today. Tomorrow it may use
something else. The compatibility profile database, the sandbox model, and the
manifest-driven architecture exist specifically so that OpenNTX can evolve its
own runtime without depending on any single external compatibility layer.

---

## License

OpenNTX is licensed under the
[PolyForm Noncommercial License 1.0.0](LICENSE).
See [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md) for commercial use.

---

## Release Notes

### v3.0.0-alpha — Consumer Edition: 1-Click Install, Systemd, Headless Daemon

#### Overview

OpenNTX v3.0.0-alpha transforms the platform from a developer tool into a
**consumer-ready product**. `sudo dpkg -i openntx.deb` is all you need — the
systemd daemon auto-starts, the GUI appears in the application menu, and
`.exe` files work immediately via `binfmt_misc`.

#### New Features

**Systemd Integration**
- `openntx-core.service` systemd unit file auto-starts the API server on boot.
- `postinst`: `systemctl daemon-reload` → `enable` → `start` on install.
- `prerm`: `systemctl stop` → `disable` → `daemon-reload` on removal.
- `Type=simple`, `Restart=on-failure`, `RestartSec=3`. No watchdog.

**Headless Daemon Mode**
- AppPortal detects non-interactive environments via `std::io::IsTerminal`.
- When no TTY is attached (systemd, Docker, SSH), starts in headless mode:
  spawns Axum API server on `127.0.0.1:8080`, parks main thread with
  `std::future::pending()`. No crash loops, no ENXIO errors.

**GUI Auto-Fallback Launcher**
- `openntx-gui` probes `http://127.0.0.1:8080` on startup.
- If API server is not running, auto-spawns `/usr/bin/openntx-appportal`
  as a detached background process. Users never need to start the daemon manually.

**Native File Dialog & Drag-and-Drop**
- `rfd` crate for native XDG Desktop Portal file picker (`.exe` filter).
- `winit` drag-and-drop via `WinitWindowAccessor::on_winit_window_event`.
- Central Drop Zone UI with Browse button and DEPLOY EXECUTION TARGET.

**Updated Dependencies (`Depends:`)**
- `wine-binfmt | wine` — Required for .exe execution.
- `cgroup-tools` — Required for sandbox resource limits.
- `systemd` — Required for daemon lifecycle management.

**Desktop Entry**
- `openntx.desktop` installed to `/usr/share/applications/`.
- MIME type `application/x-ms-dos-executable` for .exe file association.
- `Exec=/usr/bin/openntx-gui %F` — double-click .exe opens GUI.

#### Quality

- 0 errors, 0 warnings. 339/339 tests pass. `cargo fmt` clean.

### v2.8.0-alpha — Automated Debian Package Generation Engine

#### Overview

OpenNTX v2.8.0-alpha introduces the **Automated Debian Package Generation
Engine** (`openntx-packaging`), a complete, production-ready system for building
distributable `.deb` packages of the entire OpenNTX platform. This release
transforms OpenNTX from a source-only project into a first-class
Debian-distributable application subsystem for Linux.

#### New Features

**System-Level .deb Packaging Engine**
- Full platform packaging: builds a single `.deb` containing all OpenNTX
  binaries (`openntx`, `openntx-gui`, `openntx-appportal`), default
  configuration, and maintainer scripts.
- Rust-native engine (`crates/openntx-core/src/packaging/system_deb.rs`):
  ~500 lines of production Rust code implementing the complete packaging
  pipeline — from `cargo build --release` through staging layout assembly
  to `dpkg-deb --build` invocation.
- 8 comprehensive unit tests covering control file format, postinst/prerm
  script structure, TOML validation, binary target completeness, and
  dependency verification.

**Maintainer Scripts (DEBIAN/)**
- `postinst` (Post-Installation): Automatically creates the `openntx` system
  user/group, establishes `/var/lib/openntx/sandboxes` with proper
  ownership/permissions, registers the PE (MZ) executable format with
  the Linux kernel's `binfmt_misc` subsystem, and initializes the cgroups v2
  hierarchy at `/sys/fs/cgroup/openntx/` with CPU and memory controllers.
- `prerm` (Pre-Removal): Implements the **Reaper Engine** — gracefully
  terminates all managed Windows processes by reading PIDs from per-app
  cgroup `cgroup.procs` files, sending SIGTERM with a 500ms grace period,
  then escalating to SIGKILL for survivors. Unregisters the `binfmt_misc`
  handler, stops daemon services, and cleans up the cgroup hierarchy.

**Default Sandbox Configuration (`etc/openntx/sandbox.toml`)**
- Cgroups v2 resource limits: 2 GiB memory cap, 50% CPU bandwidth quota,
  256 PID limit per application sandbox.
- Network jail: default isolation mode `none` (no network access) with
  configurable modes: `host`, `bridge` (10.200.0.0/24), `isolated` (loopback).
- Filesystem isolation: private mount propagation, optional `/tmp` bind-mount,
  font directory passthrough.
- Process isolation: capability dropping, configurable nice value.
- Reaper Engine tuning: 500ms kill grace period, 5s reap interval.

**Build Automation (`tools/build-deb.sh`)**
- 9-step automated pipeline: environment validation → release binary
  compilation → binary validation → staging assembly → binary installation →
  control/postinst/prerm generation → config installation → permission
  normalization → `dpkg-deb --build`.
- Supports `--dry-run`, `--skip-build`, `--version`, `--output` flags.

**CLI Integration**
- `openntx system-package build` — Build the complete system `.deb` package.
- `openntx system-package plan` — Preview package layout without building.

#### Package Layout

```text
target/debian/openntx_3.0.0_amd64.deb
├── DEBIAN/
│   ├── control           Package: openntx, Depends: wine, cgroup-tools, systemd
│   ├── postinst          User creation, binfmt, cgroups, systemd enable/start
│   └── prerm             Reaper Engine, systemd disable/stop, cgroup cleanup
├── usr/bin/
│   ├── openntx           CLI binary
│   ├── openntx-gui       Slint GUI binary (drag-drop, native file dialog)
│   └── openntx-appportal App Portal binary (API server + headless daemon)
├── usr/lib/systemd/system/
│   └── openntx-core.service   Systemd unit (Type=simple, Restart=on-failure)
├── usr/share/applications/
│   └── openntx.desktop         Desktop entry (Exec=openntx-gui, MIME .exe)
├── etc/openntx/
│   └── sandbox.toml      Default sandbox configuration
└── var/lib/openntx/
    └── sandboxes/         Runtime sandbox directory
```

#### Dependencies

- Required: `libc6 (>= 2.31)`, `libx11-6`, `libgcc-s1 (>= 3.0)`, `libstdc++6 (>= 11)`, `wine-binfmt | wine`, `cgroup-tools`, `systemd`
- Recommended: `xdg-utils`, `xdg-desktop-portal`

#### Files Changed/Created

| File | Status | Description |
|---|---|---|
| `crates/openntx-core/src/packaging/system_deb.rs` | NEW | System-level .deb packaging engine |
| `crates/openntx-core/src/packaging/mod.rs` | MODIFIED | Added system_deb exports |
| `crates/openntx-core/src/error.rs` | MODIFIED | Added `SystemDebBuild` variant |
| `etc/openntx/sandbox.toml` | NEW | Default sandbox configuration |
| `tools/build-deb.sh` | NEW | Automated build script |
| `crates/openntx-cli/src/commands/system_package.rs` | NEW | CLI command |
| `crates/openntx-cli/src/commands/mod.rs` | MODIFIED | Registered command |
| `crates/openntx-cli/src/output.rs` | MODIFIED | Added success/info helpers |

#### Usage

```bash
# Build the complete .deb package
./tools/build-deb.sh --version 2.8.0

# Via CLI
cargo run -p openntx-cli -- system-package build --version 2.8.0

# Dry-run (preview layout)
cargo run -p openntx-cli -- system-package build --dry-run

# Show package plan
cargo run -p openntx-cli -- system-package plan
```

#### Quality Assurance

- 0 compilation errors, 0 warnings across the entire workspace.
- All existing tests preserved — no regressions introduced.
- Code formatting: fully compliant with `rustfmt` standards.

---

### Previous Releases

See [RELEASE_NOTES.md](RELEASE_NOTES.md) and [CHANGELOG.md](CHANGELOG.md)
for the complete release history from v1.0-alpha through v2.7.0-alpha.
