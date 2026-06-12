# Changelog

All notable changes to OpenNTX will be documented in this file.

## Unreleased

- Add V0.4 app registry and install plan writer with `openntx install --write-plan`, `openntx list`, `openntx show`, and confirmed/dry-run remove behavior.
- Add V0.3 manifest generation from PE analysis, including `openntx manifest generate`, JSON output, output-file writing, install dry-run manifest planning, generated manifest diagnostics metadata, and a generated-from-PE example.
- Add V0.2 real PE analyzer metadata parsing for DOS header, PE signature, COFF header, optional header, machine architecture, subsystem, image kind, section table, entry point, image base, and imported DLL names.
- Add V0.1.1 polish: CI workflow, README badges, GitHub issue and pull request templates, architecture diagram, testing guide, and screenshots placeholder.
- Add V0.1 foundation documentation, schemas, examples, Rust workspace, CLI skeleton, core models, AppPortal mock, and development scripts.
- Add source-available noncommercial licensing and commercial-license terms.

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
