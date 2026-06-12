# Manifest Specification

Every installed OpenNTX application is described by a manifest.

Default user manifest path:

```text
~/.local/share/openntx/apps/<app-id>/manifest.json
```

## Fields

- `schema_version`: OpenNTX manifest schema version.
- `app_id`: stable lowercase app identifier.
- `name`: display name.
- `version`: application version when known.
- `source`: original installer or portable executable metadata.
- `executable`: Windows path, arguments, and working directory.
- `architecture`: `x86`, `x86_64`, `arm64`, or `unknown`.
- `install_mode`: `portable`, `captured`, `planned`, or `run-once`.
- `windows_compatibility`: requested Windows version and DPI mode.
- `filesystem`: drive C root and Linux host access mapping.
- `registry`: registry overlay mode and hive paths.
- `sandbox`: permission profile and per-resource permissions.
- `graphics`: graphics backend preference and future translation metadata.
- `audio`: audio backend preference.
- `desktop`: Linux launcher and menu metadata.
- `packaging`: future package metadata.
- `diagnostics`: logging, crash report preferences, and optional PE-derived metadata such as imported DLL names, entry point RVA, and image base.

## V0.5 Generation

OpenNTX V0.5 can generate a manifest from PE analysis:

```bash
openntx manifest generate app.exe
openntx manifest generate app.exe --json
openntx manifest generate app.exe --output /tmp/app.openntx.json
```

Generated manifests are analysis-only metadata. The command does not run the EXE, does not run installers, and does not invoke external compatibility tools.

Generation rules:

- installer-looking filenames generate `install_mode = "captured"`.
- Windows GUI executables generate `install_mode = "captured"` as a future installer/capture-oriented plan.
- console or portable-looking executables generate `install_mode = "portable"`.
- imported DLL names are stored in `diagnostics.imported_dlls` when available.
- entry point RVA and image base are stored in diagnostics when available.
- sandbox defaults use the standard profile with network/documents/downloads set to ask and home/removable drives denied.

## V0.5 Registry Writes

`openntx install app.exe --write-plan` writes:

```text
~/.local/share/openntx/apps/<app-id>/
  manifest.json
  install-plan.json
  metadata.json
  drive_c/
  registry/
  logs/
```

This is still metadata and directory preparation only. No Windows code is executed.

With `--desktop`, V0.5 also writes:

```text
~/.local/share/applications/openntx-<app-id>.desktop
```

The desktop launcher uses `Exec=openntx run <app-id>`. Runtime execution remains a not-implemented backend.

## Policy

Manifests are data contracts. Runtime behavior should be derived from the manifest and compatibility profile, not hidden inside UI code.
