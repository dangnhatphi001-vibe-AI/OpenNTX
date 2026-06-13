# OpenNTX v0.9.0 — Debian Package Builder Prototype

## Highlights

- **Debian .deb package builder** — build root-owned `.deb` packages from registered OpenNTX apps with correct file permissions (0644 for files, 0755 for directories).
- **Installer capture snapshot/diff infrastructure** — snapshot app directory state, compute filesystem diffs, generate capture reports.
- **AppPortal capture actions** — Snapshot Before/After, Diff, Report, Status from the terminal UI.
- **Security hardening** — `symlink_metadata()` for safe file inspection, `--root-owner-group` for package ownership, staging in temp dir for reliable permissions on all filesystems.

## What Works Today

- Analyze real Windows PE/EXE files (headers, sections, entry point, imported DLLs).
- Generate OpenNTX app manifests from PE metadata.
- Register apps locally with manifest, install plan, metadata, and per-app state directories.
- Create and remove Linux `.desktop` launchers for registered apps.
- Run-plan diagnostics with real app metadata, JSON logs, and desktop notifications.
- Capture snapshot/diff infrastructure for OpenNTX app directories.
- Build `.deb` packages with proper permissions and root ownership.
- AppPortal TUI for browsing, analyzing, capturing, and packaging.

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
- No Windows binary, installer, or external runtime is ever executed.

## Manual Test Commands

```bash
# Install the CLI
cargo install --path crates/openntx-cli --force

# Register an app
openntx install ~/Downloads/setup.exe --write-plan

# List registered apps
openntx list

# Capture workflow
openntx capture snapshot-before <app-id>
openntx capture snapshot-after <app-id>
openntx capture diff <app-id>
openntx capture report <app-id>

# Build a .deb package
openntx package build <app-id> --yes

# Inspect the package
dpkg-deb -I dist/*.deb
dpkg-deb -c dist/*.deb | head -100
```

## Known Limitations

- Runtime execution is not implemented; `openntx run` produces a plan only.
- The `.deb` package builder is a prototype; it does not embed icons or handle all edge cases.
- AppPortal is a terminal UI; no GTK/libadwaita frontend yet.
- Capture reports are analysis-only; no real installer execution occurs.
- No Wine, Proton, Bottles, or Lutris integration.

## Next Milestone: V1.0-alpha Polish

- README and documentation polish.
- GitHub release with CI, visual assets, and demo documentation.
- AppPortal screenshots and terminal captures.
- No new runtime execution planned for V1.0-alpha.
