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
- `diagnostics`: logging and crash report preferences.

## Policy

Manifests are data contracts. Runtime behavior should be derived from the manifest and compatibility profile, not hidden inside UI code.
