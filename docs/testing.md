# Testing OpenNTX V0.1

OpenNTX V0.1 is a foundation release. Tests verify static analysis, schemas, CLI planning, AppPortal mock output, and core data models. They do not verify Windows application execution because runtime execution is not implemented.

## Prerequisites

Install Rust stable:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup toolchain install stable --profile minimal --component rustfmt
rustup default stable
```

Optional JSON Schema validator:

```bash
python3 -m pip install --user jsonschema
```

## Required Checks

Run from the repository root:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
tools/dev-check.sh
```

`tools/dev-check.sh` runs formatting, workspace tests, JSON syntax checks, and optional schema validation when Python `jsonschema` is installed.

## CLI Smoke Tests

Run the package planner:

```bash
cargo run -p openntx-cli -- package example-app
```

Run the mock install flow:

```bash
tools/mock-install-flow.sh
```

The mock flow creates a temporary minimal PE fixture and demonstrates analysis and install planning. It does not run the EXE.

## AppPortal Smoke Test

```bash
cargo run -p openntx-appportal
```

Expected result: a text UI/mock showing the home screen, drop zone, install wizard, app library, settings, and CLI bridge preview.

## What V0.1 Tests Do Not Cover

- Windows process execution.
- Installer execution.
- Real filesystem capture.
- Registry emulation.
- Win32 or NT API compatibility.
- DirectX or graphics translation.
- Kernel drivers.
- Protected software or anti-cheat scenarios.
