# Contributing to OpenNTX

OpenNTX is a source-available systems project. Contributions are welcome when they preserve the architecture, security posture, and honest compatibility claims of the project.

Before contributing, read:

- [LICENSE](LICENSE)
- [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md)
- [CONTRIBUTOR-LICENSE-TERMS.md](CONTRIBUTOR-LICENSE-TERMS.md)

By submitting a contribution, you agree to the contributor license terms. In particular, contributors grant the OpenNTX project owner a broad license to use, modify, distribute, sublicense, and commercially license contributions.

## Coding Style

- Keep core logic independent from AppPortal UI code.
- Prefer small, typed modules over large scripts.
- Use Rust `Result` types and explicit error messages.
- Avoid hidden runtime behavior in GUI code.
- Keep placeholders clearly marked as TODO or future modules.
- Use `cargo fmt --all` before submitting.

## Branch Naming

Use short, scoped names:

- `feature/pe-analyzer`
- `docs/runtime-design`
- `fix/manifest-validation`
- `test/desktop-entry-generation`

## Issue Format

Use clear technical issue reports:

- current behavior
- expected behavior
- reproduction steps
- logs or command output
- affected version or commit
- security impact, if any

## Architecture-First Contributions

OpenNTX is not a launcher wrapper. Changes should respect:

- core-first design
- manifest-driven behavior
- per-app isolation
- desktop-native integration
- security by default
- runtime abstraction

Large runtime or installer-capture changes should start with a design issue or architecture note.

## Compatibility Claims

Do not claim that OpenNTX runs arbitrary Windows applications unless that support exists and is covered by tests or reproducible compatibility profiles.

Acceptable wording:

- "analysis-only"
- "dry-run"
- "future runtime backend"
- "prototype"
- "experimental"

Unacceptable wording:

- "runs all EXE files"
- "native Windows runtime complete"
- "drop any app and it works"

## Tests

Tests are required for:

- manifest schema or validation changes
- PE analyzer behavior
- desktop entry generation
- app-id generation
- sandbox policy defaults
- capture report models

Run:

```bash
cargo test --workspace
tools/dev-check.sh
```

## Runtime Module Proposals

New runtime modules should document:

- target Windows subsystem or API family
- Linux backend dependencies
- security model
- process model
- unsupported cases
- diagnostics and logging
- test fixtures

Do not add code that bypasses DRM, anti-cheat, credential protections, sandboxing, or user consent.
