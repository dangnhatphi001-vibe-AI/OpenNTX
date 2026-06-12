# Testing OpenNTX V0.5

OpenNTX V0.5 is a foundation release with real PE metadata analysis, manifest generation, local app registry plan writing, and desktop launcher writing. Tests verify static analysis, schemas, CLI planning, AppPortal mock output, core data models, PE headers, section tables, subsystem detection, entry point, image base, imported DLL names, generated manifest validation, registry directory creation, list/show, remove dry-run/delete behavior, and desktop launcher create/remove behavior. They do not verify Windows application execution because runtime execution is not implemented.

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

Generate manifest metadata:

```bash
cargo run -p openntx-cli -- manifest generate app.exe
cargo run -p openntx-cli -- manifest generate app.exe --json
cargo run -p openntx-cli -- manifest generate app.exe --output /tmp/app.openntx.json
```

`--json` prints pretty JSON only, with no extra prose.

Write a local registry plan without touching your real app registry:

```bash
export XDG_DATA_HOME="$(mktemp -d)"
cargo run -p openntx-cli -- install app.exe --write-plan
cargo run -p openntx-cli -- install app.exe --write-plan --desktop
cargo run -p openntx-cli -- list
cargo run -p openntx-cli -- show <app-id>
cargo run -p openntx-cli -- desktop create <app-id> --dry-run
cargo run -p openntx-cli -- desktop create <app-id> --yes
cargo run -p openntx-cli -- desktop remove <app-id> --dry-run
cargo run -p openntx-cli -- desktop remove <app-id> --yes
cargo run -p openntx-cli -- remove <app-id> --dry-run
cargo run -p openntx-cli -- remove <app-id> --yes
```

`openntx install app.exe` remains dry-run unless `--write-plan` is passed.

## AppPortal Smoke Test

```bash
cargo run -p openntx-appportal
```

Expected result: a text UI/mock showing the home screen, drop zone, install wizard, app library, settings, and CLI bridge preview.

## What V0.5 Tests Do Not Cover

- Windows process execution.
- Installer execution.
- Real filesystem capture.
- Registry emulation.
- Win32 or NT API compatibility.
- DirectX or graphics translation.
- Kernel drivers.
- Protected software or anti-cheat scenarios.
