# Changelog

All notable changes to OpenNTX will be documented in this file.

## Unreleased

- Fix V0.7 run-plan CLI: resolve stale binary issue where `--json`, `--notify`, `--no-log` flags were not recognized by the installed `openntx` binary.
- Unify install-mode heuristic across `analyze`, `manifest generate`, and `install --write-plan` so non-installer GUI tools (like CPU-Z) consistently get `run-once`/`portable` instead of the previous inconsistent `captured`.
- Add `target` and `timestamp` fields to run-plan reports.
- Add `install_mode_reason` to PE analysis output and CLI `analyze` display.
- Add tests for heuristic consistency, run-plan JSON round-trip, no-log behavior, desktop Exec `--notify`, notify-send safety, and V0.7 status messaging.
- Update AppPortal run-plan screen with real app name, target, and timestamp.
- Add V0.5 desktop launcher writer with `openntx desktop create/remove`, `install --write-plan --desktop`, launcher status in list/show, and desktop create/remove tests.
- Add V0.4 app registry and install plan writer with `openntx install --write-plan`, `openntx list`, `openntx show`, and confirmed/dry-run remove behavior.
- Add V0.3 manifest generation from PE analysis, including `openntx manifest generate`, JSON output, output-file writing, install dry-run manifest planning, generated manifest diagnostics metadata, and a generated-from-PE example.
- Add V0.2 real PE analyzer metadata parsing for DOS header, PE signature, COFF header, optional header, machine architecture, subsystem, image kind, section table, entry point, image base, and imported DLL names.
- Add V0.1.1 polish: CI workflow, README badges, GitHub issue and pull request templates, architecture diagram, testing guide, and screenshots placeholder.
- Add V0.1 foundation documentation, schemas, examples, Rust workspace, CLI skeleton, core models, AppPortal mock, and development scripts.
- Add source-available noncommercial licensing and commercial-license terms.

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
