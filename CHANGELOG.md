# Changelog

All notable changes to OpenNTX will be documented in this file.

## Unreleased

- Add V0.2 real PE analyzer metadata parsing for DOS header, PE signature, COFF header, optional header, machine architecture, subsystem, image kind, section table, entry point, image base, and imported DLL names.
- Add V0.1.1 polish: CI workflow, README badges, GitHub issue and pull request templates, architecture diagram, testing guide, and screenshots placeholder.
- Add V0.1 foundation documentation, schemas, examples, Rust workspace, CLI skeleton, core models, AppPortal mock, and development scripts.
- Add source-available noncommercial licensing and commercial-license terms.

## 0.2.0 - Real PE Analyzer

- Parse real PE metadata while keeping behavior analysis-only.
- Expand `openntx analyze` output with headers, sections, entry point, image base, and imported DLLs.
- Add generated PE fixture tests for x86, x86_64, GUI, console, invalid, MZ-only, DLL image kind, and nonexistent files.

## 0.1.0 - Foundation

- Initial project foundation.
- No Windows runtime execution support.
