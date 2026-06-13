# Changelog

All notable changes to OpenNTX will be documented in this file.

## Unreleased

## 0.9.0 — Debian Package Builder Prototype

### Highlights

- Build `.deb` packages from registered OpenNTX apps with correct file permissions and root ownership.
- Installer capture snapshot/diff infrastructure for OpenNTX app directories.
- AppPortal capture actions: Snapshot Before/After, Diff, Report, Status.

### Packaging

- Add `openntx package build <app-id>` with `--yes`, `--output`, `--version`, `--dry-run` flags.
- Build `.deb` with `dpkg-deb --root-owner-group` for root/root ownership.
- Set 0644 permissions on regular files and 0755 on directories in staging.
- Stage in system temp dir for reliable permissions on all filesystems.
- Add `set_staging_permissions` helper for recursive Unix permission fixing.
- Dry-run by default; pass `--yes` to actually build.

### Capture

- Add `openntx capture snapshot-before <app-id>`.
- Add `openntx capture snapshot-after <app-id>`.
- Add `openntx capture diff <app-id>`.
- Add `openntx capture report <app-id>`.
- Add `openntx capture status <app-id>`.
- All capture commands support `--json`.
- All capture commands fail gracefully if app ID is unknown or snapshots are missing.

### AppPortal

- Add Capture menu option with per-app Snapshot Before/After, Diff, Report, Status actions.
- Add Package (.deb) action in app details.

### Documentation

- Polish README.md for V1.0-alpha GitHub presentation.
- Add RELEASE_NOTES.md for v0.9.0.
- Add docs/demo.md with step-by-step demo guide.
- Add V1.0-alpha milestone to ROADMAP.md.
- Update CHANGELOG, docs, and schemas for V0.9.

### Security

- Use `symlink_metadata()` for safe file inspection in capture and packaging.
- Reject symlinks in capture snapshots and package staging.
- Stage in temp dir to avoid permission issues on FAT/NTFS/external mounts.

## 0.8.0 - Installer Capture Snapshot/Diff Infrastructure

- Add `openntx capture snapshot-before <app-id>` to snapshot OpenNTX app directory state before installer execution.
- Add `openntx capture snapshot-after <app-id>` to snapshot OpenNTX app directory state after installer execution.
- Add `openntx capture diff <app-id>` to compute filesystem diff between before/after snapshots.
- Add `openntx capture report <app-id>` to generate a capture report from the diff.
- Add `openntx capture status <app-id>` to show capture state for a registered app.
- Add core capture modules: `snapshot.rs`, `diff.rs`, `report_writer.rs` with reusable logic.
- Snapshot entries include relative path, kind (file/directory/symlink/other), size, modified time, SHA256 hash, and readonly flag.
- Diff results include files created/removed/modified, directories created/removed, registry files changed, warnings, and errors.
- Capture reports extend the existing capture-report schema with `app_id`, `app_name`, `files_removed`, and `status` field.
- All capture commands support `--json` for machine-readable output.
- All capture commands fail gracefully if app ID is unknown or required snapshots are missing.
- Update AppPortal with Capture menu option and per-app capture actions (Snapshot Before/After, Diff, Report, Status).
- Add capture tests for snapshot creation, file created/modified/removed detection, directory created/removed detection, registry file change tracking, missing snapshot errors, report JSON validity, symlink/path traversal safety, and JSON round-trip.
- V0.8 adds capture snapshot/diff infrastructure only. It does not run Windows installers yet.

## 0.7.0 - Run Plan UX and Diagnostics

- Load registered app manifests in `openntx run <app-id>`.
- Show real app metadata in run-plan output.
- Write JSON diagnostics logs under `~/.local/state/openntx/logs/`.
- Add `openntx run <app-id> --json`.
- Add optional `openntx run <app-id> --notify` using `notify-send` when available.
- Add `openntx run <app-id> --no-log` to suppress diagnostics log.
- Update desktop launchers to call `openntx run <app-id> --notify`.
- Reuse the same run-plan logic in AppPortal.
- Unify install-mode heuristic across analyze, manifest generate, and install flows.
- Add `target`, `timestamp`, and `install_mode_reason` fields to run-plan and analysis output.
- Require `cargo install --path crates/openntx-cli` to update the installed binary.

## 0.6.0 - AppPortal Registry UI

- Read registered apps directly from the OpenNTX app registry.
- Show app details, sandbox profile, imported DLL count, manifest path, executable path, and desktop status.
- Add AppPortal Analyze EXE and Install Plan flows backed by core PE analysis and manifest generation.
- Add AppPortal desktop launcher create/remove actions with confirmation.
- Add AppPortal run-plan placeholder without executing Windows binaries.
- Add confirmed app removal from AppPortal.

## 0.5.0 - Desktop Launcher Writer

- Write user `.desktop` launchers for registered apps.
- Add `openntx desktop create <app-id>`.
- Add `openntx desktop remove <app-id>`.
- Add `openntx install <file.exe> --write-plan --desktop`.
- Keep launcher Exec pointed at `openntx run <app-id>` while runtime remains not implemented.

## 0.4.0 - App Registry and Install Plan Writer

- Write local OpenNTX app directories without executing Windows binaries.
- Write `manifest.json`, `install-plan.json`, `metadata.json`, `drive_c/`, `registry/`, and `logs/`.
- Add registered app list/show/remove flows.
- Keep `openntx install` dry-run by default; require `--write-plan` for local writes.

## 0.3.0 - Manifest Generator

- Generate OpenNTX manifests from real PE analysis results.
- Add `openntx manifest generate <file.exe>`.
- Support `--json` and `--output <path>`.
- Keep install flow dry-run and analysis-only.
- No Windows binary execution or installer execution.

## 0.2.0 - Real PE Analyzer

- Parse real PE metadata while keeping behavior analysis-only.
- Expand `openntx analyze` output with headers, sections, entry point, image base, and imported DLLs.
- Add generated PE fixture tests for x86, x86_64, GUI, console, invalid, MZ-only, DLL image kind, and nonexistent files.

## 0.1.0 - Foundation

- Initial project foundation.
- No Windows runtime execution support.
