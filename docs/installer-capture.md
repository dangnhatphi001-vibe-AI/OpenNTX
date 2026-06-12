# Installer Capture

Installer Capture Mode is the process for turning a Windows installer into an OpenNTX-managed application.

## V0.8 Implementation

V0.8 adds capture snapshot/diff infrastructure for OpenNTX-managed app directories. It does not execute installers.

### CLI Commands

```bash
openntx capture snapshot-before <app-id>
openntx capture snapshot-after <app-id>
openntx capture diff <app-id>
openntx capture report <app-id>
openntx capture status <app-id>
```

All commands support `--json` for machine-readable output.

### Data Paths

For each registered app:

```text
~/.local/share/openntx/apps/<app-id>/capture/
```

Files:

- `snapshot-before.json`
- `snapshot-after.json`
- `capture-diff.json`
- `capture-report.json`

### Snapshot Scope

V0.8 snapshots only the OpenNTX app directory:

- `drive_c/`
- `registry/`
- optionally `metadata.json`
- optionally `manifest.json`
- optionally `install-plan.json`

It does not scan the entire home directory, system directories, or files outside the OpenNTX app directory.

### Snapshot Entry Fields

Each snapshot entry includes:

- relative path
- kind: file / directory / symlink / other
- size
- modified time (unix seconds) if available
- sha256 for regular files
- readonly flag if available

### Diff Result Fields

- files_created
- files_removed
- files_modified
- directories_created
- directories_removed
- registry_files_changed
- warnings
- errors

### Capture Report

The capture report includes:

- schema_version
- capture_id
- app_id
- app_name
- input_installer (from manifest)
- started_at / finished_at (snapshot timestamps)
- files_created / files_modified / files_removed
- registry_keys_created (placeholder)
- registry_values_changed (placeholder)
- shortcuts_detected (placeholder)
- executable_candidates (placeholder)
- warnings / errors
- status: "analysis-only / no runtime execution"

### Security Rules

- Does not execute Windows binaries.
- Does not run installers.
- Does not scan outside the registered app directory.
- Does not follow symlinks outside the app directory.
- Prevents path traversal.
- Keeps capture analysis-only.
- Rejects symlinked `manifest.json`, `metadata.json`, `install-plan.json` before reading content.
- Rejects symlinked `drive_c/`, `registry/`, `capture/` directories.
- Uses `symlink_metadata()` everywhere — never follows symlinks during snapshot scanning.
- Re-checks file type immediately before hashing (TOCTOU mitigation).
- Validates all artifact write paths remain inside the app directory.

### Manual Security Probe Checklist

To verify symlink hardening manually:

```bash
# 1. Register a test app
openntx install ./test.exe --write-plan
APP_ID="<app-id>"

# 2. Replace manifest.json with symlink to outside file
ln -sf ~/.bashrc ~/.local/share/openntx/apps/$APP_ID/manifest.json

# 3. Verify capture commands fail safely (no JSON parse error, clean symlink rejection)
openntx capture snapshot-before $APP_ID
# Expected: "unsafe file: manifest.json must be a regular file, not a symlink"

openntx capture status $APP_ID
# Expected: same symlink rejection error

# 4. Verify list skips the corrupted app
openntx list
# Expected: the corrupted app is silently skipped

# 5. Verify show fails safely
openntx show $APP_ID
# Expected: symlink rejection error

# 6. Restore and verify normal operation
rm ~/.local/share/openntx/apps/$APP_ID/manifest.json
openntx install ./test.exe --write-plan
openntx capture snapshot-before $APP_ID
# Expected: success

# 7. Repeat with install-plan.json and metadata.json
ln -sf ~/.bashrc ~/.local/share/openntx/apps/$APP_ID/install-plan.json
openntx capture snapshot-before $APP_ID
# Expected: install-plan.json is rejected as symlink (skipped in snapshot)
```

Each probe uses a fresh app registry entry. If a probe corrupts state, remove the app directory and re-register before the next probe.

## Planned Future Flow

1. Create a temporary install environment.
2. Snapshot filesystem state.
3. Snapshot registry overlay state.
4. Run installer inside a controlled runtime backend.
5. Snapshot filesystem state again.
6. Snapshot registry overlay state again.
7. Diff filesystem changes.
8. Diff registry changes.
9. Detect shortcuts.
10. Identify candidate main executables.
11. Generate a capture report.
12. Generate or refine an app manifest.
13. Generate a Linux launcher.
14. Optionally package the captured app into a `.deb`.

## Capture Report

Capture reports are structured JSON documents containing:

- files created
- files modified
- registry keys created
- registry values changed
- shortcuts detected
- executable candidates
- installer exit code
- warnings
- errors

## V0.8 Limitation

V0.8 implements snapshot/diff infrastructure for OpenNTX app directories only. It still does not execute installers or perform real filesystem/registry diff capture of installer output.
