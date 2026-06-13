# OpenNTX v1.0.0-alpha — V1.0-alpha Feature Pack

## Highlights

- **App management** — rename, duplicate, export/import bundles with safety checks.
- **Doctor / integrity checks** — global and per-app health diagnostics with safe repair.
- **Logs management** — list, show, and clean run-plan logs.
- **Config system** — persistent configuration via `openntx config`.
- **Shell completions** — bash, zsh, fish via `openntx completions`.
- **AppPortal V1.0-alpha** — redesigned TUI with Library, Capture, Package Builder, Logs, Doctor, Settings.
- **33 new tests** covering security boundaries and all new features.

## What Works Today

- Analyze real Windows PE/EXE files (headers, sections, entry point, imported DLLs).
- Generate OpenNTX app manifests from PE metadata.
- Register apps locally with manifest, install plan, metadata, and per-app state directories.
- Create and remove Linux `.desktop` launchers for registered apps.
- Run-plan diagnostics with real app metadata, JSON logs, and desktop notifications.
- Capture snapshot/diff infrastructure for OpenNTX app directories.
- Build `.deb` packages with proper permissions and root ownership.
- Rename, duplicate, export, and import registered apps.
- Global and per-app diagnostics with safe repair.
- List, show, and clean run-plan logs.
- Persistent configuration file.
- Shell completions for bash, zsh, fish.
- AppPortal TUI with comprehensive app management.

## What Remains Analysis-Only

- Windows binary execution — **not implemented**.
- Installer execution — **not implemented**.
- Win32/NT runtime — **not implemented**.
- DirectX translation — **not implemented**.
- Driver support — **not implemented**.

## Security Notes

- PE analysis reads headers and metadata only; no code is executed.
- Capture snapshots inspect OpenNTX app directories only.
- The `.deb` package builder uses `symlink_metadata()` to reject symlinks, sets 0644/0755 permissions, stages in a temp dir, and builds with `--root-owner-group`.
- Bundle export/import rejects path traversal and symlink escapes.
- Doctor repair refuses to follow unsafe symlinks.
- All destructive operations are dry-run by default; `--yes` required for writes.
- No Windows binary, installer, or external runtime is ever executed.

## New CLI Commands

```
openntx rename <app-id> <new-name> [--yes]
openntx duplicate <app-id> --as <new-app-id> [--yes]
openntx export <app-id> --output <path> [--yes]
openntx import <bundle-path> [--as <new-app-id>] [--yes]
openntx list --json
openntx show <app-id> --json
openntx doctor [--json]
openntx doctor <app-id> [--json] [--repair --yes]
openntx logs list [--json]
openntx logs show <path-or-app-id> [--json]
openntx logs clean --older-than-days <N> [--yes]
openntx capture clean <app-id> [--yes]
openntx capture diff <app-id> --summary
openntx capture report <app-id> --json
openntx capture status <app-id> --json
openntx package inspect <deb-file>
openntx package clean [--yes]
openntx package build <app-id> --keep-staging [--yes]
openntx config show
openntx config init
openntx config set <key> <value>
openntx config reset [--yes]
openntx completions <shell>
```

## Manual Test Commands

```bash
# Install the CLI
cargo install --path crates/openntx-cli --force

# Register an app
openntx install ~/Downloads/setup.exe --write-plan

# List registered apps (JSON)
openntx list --json

# Show app details (JSON)
openntx show <app-id> --json

# Doctor
openntx doctor
openntx doctor <app-id>
openntx doctor <app-id> --repair --yes

# Logs
openntx logs list
openntx logs show <app-id>

# App management
openntx rename <app-id> "New Name" --yes
openntx duplicate <app-id> --as <new-id> --yes
openntx export <app-id> --output backup.tar.gz --yes
openntx import backup.tar.gz --yes

# Build a .deb package
openntx package build <app-id> --yes
openntx package inspect dist/*.deb

# Config
openntx config show
openntx config init

# Shell completions
openntx completions bash
```

## Known Limitations

- Runtime execution is not implemented; `openntx run` produces a plan only.
- The `.deb` package builder is a prototype; it does not embed icons or handle all edge cases.
- AppPortal is a terminal UI; no GTK/libadwaita frontend yet.
- Capture reports are analysis-only; no real installer execution occurs.
- No Wine, Proton, Bottles, or Lutris integration.
- Config uses JSON (not TOML) to avoid additional dependencies.

## Next Milestone: V1.1

- AppPortal screenshots and terminal captures.
- Visual assets refresh.
- GitHub release with tags and release notes.
- No new runtime execution planned.
