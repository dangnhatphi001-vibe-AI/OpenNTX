# OpenNTX Demo Guide

This guide walks through the real V1.0-alpha analysis and packaging flow using a Windows PE/EXE file.

**OpenNTX does not execute Windows binaries.** Every step is analysis, metadata, or packaging only.

## Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Build and install the CLI
cargo install --path crates/openntx-cli --force
```

## Example Files

You can use any Windows PE/EXE installer for this demo. Examples:

- **CPU-Z** — `cpuz_x64.exe` (small system info tool)
- **Notepad++** — `npp.Installer.x64.exe` (text editor)
- **TLauncher** — `TLauncher-setup.exe` (Minecraft launcher)

Any valid Windows PE/EXE file will work. OpenNTX reads metadata only; it does not run the binary.

## Step 1: Analyze the PE File

```bash
openntx analyze ~/Downloads/cpuz_x64.exe
```

This prints PE metadata: headers, sections, architecture, subsystem, entry point, imported DLLs.

## Step 2: Generate a Manifest

```bash
openntx manifest generate ~/Downloads/cpuz_x64.exe --json
```

This converts PE metadata into an OpenNTX app manifest. Use `--output /tmp/app.openntx.json` to save it.

## Step 3: Register the App

```bash
openntx install ~/Downloads/cpuz_x64.exe --write-plan
```

This creates a local app directory under `~/.local/share/openntx/apps/<app-id>/` with:

- `manifest.json`
- `install-plan.json`
- `metadata.json`
- `drive_c/`
- `registry/`
- `logs/`

No Windows binary is executed.

## Step 4: List Registered Apps

```bash
openntx list
```

Shows all registered apps with their IDs, names, and status.

## Step 5: Create a Desktop Launcher

```bash
openntx desktop create <app-id> --yes
```

This writes a `.desktop` file to `~/.local/share/applications/` so the app appears in your Linux application menu.

## Step 6: Capture Snapshot/Diff

This demonstrates the capture infrastructure. It snapshots the app directory before and after a change, then computes a diff.

```bash
# Snapshot before
openntx capture snapshot-before <app-id>

# Manually create or modify a file in the app directory
echo "test" > ~/.local/share/openntx/apps/<app-id>/drive_c/test-file.txt

# Snapshot after
openntx capture snapshot-after <app-id>

# Compute the diff
openntx capture diff <app-id>

# Generate a capture report
openntx capture report <app-id>
```

The diff and report are saved to `~/.local/share/openntx/apps/<app-id>/capture/`.

## Step 7: Build a .deb Package

```bash
openntx package build <app-id> --yes
```

This builds a `.deb` package in `dist/` with:

- Root-owned contents (`--root-owner-group`)
- 0644 permissions on regular files
- 0755 permissions on directories
- A generated `DEBIAN/control` file
- A `.desktop` entry in `usr/share/applications/`

## Step 8: Inspect the Package

```bash
# Show package info
dpkg-deb -I dist/*.deb

# List package contents with permissions
dpkg-deb -c dist/*.deb | head -100
```

Verify:

- Version shows `1.0.0-alpha` (or your specified version).
- Owner is `root/root`.
- `.json` and `.desktop` files show `-rw-r--r--` (0644).
- Directories show `drwxr-xr-x` (0755).

## Step 9: Doctor Diagnostics

```bash
# Global health check
openntx doctor

# Per-app check
openntx doctor <app-id>

# Repair missing directories
openntx doctor <app-id> --repair --yes
```

## Step 10: App Management

```bash
# Rename an app
openntx rename <app-id> "My App Name" --yes

# Duplicate an app
openntx duplicate <app-id> --as <new-app-id> --yes

# Export as bundle
openntx export <app-id> --output backup.tar.gz --yes

# Import a bundle
openntx import backup.tar.gz --yes
```

## Step 11: Logs

```bash
# List run-plan logs
openntx logs list

# Show latest log for an app
openntx logs show <app-id>

# Clean old logs
openntx logs clean --older-than-days 30 --yes
```

## Step 12: Configuration

```bash
# Show current config
openntx config show

# Initialize config file
openntx config init

# Set a value
openntx config set log_retention_days 60
```

## AppPortal Demo

You can also use the terminal UI:

```bash
cargo run -p openntx-appportal
```

From AppPortal:

1. Open **Library** to view registered apps.
2. Use **Analyze EXE** to inspect a PE file and preview a manifest.
3. Use **Write Install Plan** to register app metadata after confirmation.
4. Use **Capture Tools** to run snapshot/diff/report per app.
5. Use **Package Builder** to build .deb packages.
6. Use **Logs** to view run-plan logs.
7. Use **Doctor** to run diagnostics.
8. Use **Settings** to view paths and defaults.

## What This Demo Does Not Do

- It does not execute the Windows binary.
- It does not run the installer.
- It does not start a Win32/NT runtime.
- It does not call Wine, Proton, Bottles, or Lutris.

Every step is analysis, metadata generation, filesystem inspection, or Linux packaging only.

## Further Reading

- [docs/architecture.md](architecture.md) — system architecture
- [docs/manifest-spec.md](manifest-spec.md) — manifest schema
- [docs/installer-capture.md](installer-capture.md) — capture design
- [docs/packaging.md](packaging.md) — packaging design
- [docs/testing.md](testing.md) — testing guide
