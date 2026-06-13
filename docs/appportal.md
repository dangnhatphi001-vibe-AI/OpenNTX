# AppPortal

AppPortal is the user-friendly frontend for OpenNTX. It is not the runtime and must not contain compatibility-layer logic.

V1.0-alpha provides a lightweight terminal UI that reads the real OpenNTX app registry from:

```text
~/.local/share/openntx/apps/
```

It does not run EXE files, run installers, or call external compatibility tools.

## Home

The home screen shows:

- OpenNTX version.
- Registered app count.
- Runtime status: not implemented (analysis-only).
- CI/status: V1.0-alpha feature pack.
- Actions:
  1. Library
  2. Analyze EXE
  3. Write Install Plan
  4. Capture Tools
  5. Package Builder
  6. Logs
  7. Doctor
  8. Settings
  Q. Quit

## App Library

The library lists registered apps with:

- app name
- app id
- architecture
- install mode
- sandbox profile
- imported DLL count
- desktop launcher status
- registry status

Selecting an app opens App Details.

## App Details

App Details shows:

- app id
- name
- architecture
- install mode
- executable path
- manifest path
- sandbox profile
- imported DLL count
- desktop entry path
- desktop status
- runtime status

Available actions:

- Run plan.
- Create desktop launcher.
- Remove desktop launcher.
- Capture: Snapshot Before.
- Capture: Snapshot After.
- Capture: Diff.
- Capture: Report.
- Capture: Status.
- Package (.deb) — dry-run plan first, build only after confirmation.
- Show logs — latest run-plan log for the app.
- Doctor — per-app diagnosis.
- Dry-run remove app.
- Remove app with explicit app-id confirmation.
- Back.

Run plan remains honest: runtime execution is not implemented. AppPortal uses the same core run-plan logic as `openntx run <app-id>` and writes a diagnostics log under:

```text
~/.local/state/openntx/logs/
```

## Doctor

The Doctor screen provides:

- **Global diagnosis** — data directory, app count, broken apps, logs directory, desktop entries, dpkg-deb availability, notify-send availability.
- **Per-app diagnosis** — manifest validity, install plan, drive_c, registry, capture directory, desktop entry, symlinks, and warnings.
- **Safe repair** (dry-run) — shows what repairs would be made without applying them.

## Logs

The Logs screen lists recent run-plan logs and allows viewing details for any log.

## Packaging

The Packaging screen:

1. Lists registered apps.
2. Shows a dry-run package plan for the selected app.
3. Builds the .deb package only after explicit confirmation.

## Analyze EXE

The Analyze EXE flow asks for a file path, runs the OpenNTX PE analyzer, and shows:

- PE format
- architecture
- subsystem
- imported DLL count
- suggested install mode
- analysis-only status

The user may preview the generated manifest JSON or write an install plan after confirmation.

## Install Plan

The Install Plan flow:

1. Asks for an EXE path.
2. Analyzes PE metadata.
3. Generates an OpenNTX manifest.
4. Shows the planned registry paths.
5. Requires confirmation before writing.
6. Writes `manifest.json`, `install-plan.json`, `metadata.json`, `drive_c/`, `registry/`, and `logs/`.

No installer is executed.

## Desktop Launcher

The Desktop Launcher flow uses the same core API as the CLI:

- create launcher dry-run first
- require confirmation before writing
- remove launcher only after confirmation

Desktop entries are written to:

```text
~/.local/share/applications/openntx-<app-id>.desktop
```

The generated `Exec` line calls:

```text
openntx run <app-id> --notify
```

`openntx run` still produces a dry-run runtime plan and writes diagnostics metadata.

## Capture

The Capture flow provides per-app installer capture snapshot/diff actions:

- **Snapshot Before**: Snapshots the OpenNTX app directory (drive_c/, registry/, optional files) before a future/manual capture step.
- **Snapshot After**: Snapshots the app directory after a future/manual capture step.
- **Diff**: Computes filesystem diff between before/after snapshots.
- **Report**: Generates a capture report from the diff.
- **Status**: Shows which capture artifacts exist for the selected app.

Capture is also accessible from the main menu as option [5].

The AppPortal clearly states: "Installer execution is not implemented. Capture snapshots only inspect OpenNTX-managed app directories."

## Settings

Settings shows:

- default sandbox profile
- runtime backend
- manifest generation status
- compatibility database status
- app registry path
- desktop entry path
- diagnostics/log path

## Background Services

V0.8 does not require a heavy background daemon. Future background services must have a documented reason, narrow permissions, and clear diagnostics.
