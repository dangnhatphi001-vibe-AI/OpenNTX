# OpenNTX Import/Export

OpenNTX supports exporting registered apps as portable bundles and importing them into another installation.

## Export

```bash
openntx export <app-id> --output <path> [--yes]
```

Creates a portable `.openntx-bundle.tar.gz` bundle containing:
- `manifest.json`
- `install-plan.json`
- `metadata.json`
- `drive_c/` directory
- `registry/` directory
- `capture/` directory (if present)

**Security rules:**
- Does not include files outside the app directory
- Rejects symlinks that escape the app directory
- Dry-run by default; requires `--yes` to create the bundle

## Import

```bash
openntx import <bundle-path> [--as <new-app-id>] [--yes]
```

Imports an OpenNTX bundle into the local app registry.

**Validation:**
- Validates the manifest before import
- Rejects unsafe archive paths containing `../`
- Rejects symlinks in the archive
- If the app-id already exists, requires `--as <new-app-id>` or fails

**Safety rules:**
- Dry-run by default; requires `--yes` to import
- All paths are validated before extraction

## Examples

```bash
# Export an app (dry-run)
openntx export my-app-1234abcd --output backup.tar.gz

# Export an app (write)
openntx export my-app-1234abcd --output backup.tar.gz --yes

# Import a bundle (dry-run)
openntx import backup.tar.gz

# Import a bundle (write)
openntx import backup.tar.gz --yes

# Import under a different app-id
openntx import backup.tar.gz --as my-other-app-5678efgh --yes
```

## Bundle Format

The bundle is a gzip-compressed tar archive (`.tar.gz`). It contains the full app directory structure, preserving the layout expected by the OpenNTX registry.

## Security

- Symlinks are rejected during both export and import
- Path traversal (`../`) is rejected during import
- All paths are canonicalized and verified to stay within the app directory
- Manifest validation runs after import to ensure integrity
