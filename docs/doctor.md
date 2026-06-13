# OpenNTX Doctor

The `openntx doctor` command runs diagnostic checks on your OpenNTX installation and registered apps.

## Global Diagnosis

```bash
openntx doctor
openntx doctor --json
```

Checks:
- OpenNTX data directory exists
- Number of registered apps
- Broken apps (invalid manifest, unparseable)
- Logs directory status and writability
- Desktop entries directory status
- `dpkg-deb` availability (for package building)
- `notify-send` availability (for desktop notifications)
- Filesystem permission warnings

## Per-App Diagnosis

```bash
openntx doctor <app-id>
openntx doctor <app-id> --json
```

Checks:
- Manifest exists and is a regular file (not a symlink)
- Manifest validates against schema
- Install plan exists
- `drive_c/` exists and is a real directory
- `registry/` exists and is a real directory
- Capture directory status
- Desktop entry exists/missing
- Package build possible
- Run-plan log directory writable
- Unsafe symlinks detection

## Safe Repair

```bash
openntx doctor <app-id> --repair --yes
```

Safe repairs (dry-run by default):
- Create missing `drive_c/` directory
- Create missing `registry/` directory
- Create missing `logs/` directory
- Create missing `capture/` directory
- Regenerate desktop launcher (only if manifest is valid)

**Rules:**
- Does not follow unsafe symlinks
- If unsafe symlinks are found, repair is refused with a clear message
- Requires `--yes` to apply repairs
- Dry-run by default

## Examples

```bash
# Global health check
openntx doctor

# Check a specific app
openntx doctor my-app-1234abcd

# JSON output for scripting
openntx doctor --json
openntx doctor my-app-1234abcd --json

# Repair missing directories
openntx doctor my-app-1234abcd --repair --yes
```

## Security

- Doctor never follows symlinks outside the app directory
- Repair never creates or modifies files outside safe, known directories
- Unsafe symlinks are detected and reported clearly
- All repair actions are dry-run by default
