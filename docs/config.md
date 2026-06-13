# OpenNTX Configuration

OpenNTX uses a persistent configuration file for default settings.

## Config File Location

```
~/.config/openntx/config.json
```

## Commands

```bash
openntx config show          # Display current configuration
openntx config init          # Create config file with defaults
openntx config set <key> <value>  # Set a configuration value
openntx config reset --yes   # Reset to defaults
```

## Configuration Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `default_output_dir` | string | `"dist"` | Default output directory for packages |
| `default_sandbox_profile` | string | `"standard"` | Default sandbox profile (strict/standard/developer) |
| `enable_notifications` | bool | `true` | Enable desktop notifications |
| `log_retention_days` | number | `30` | Number of days to keep logs |
| `package_version_default` | string | `"1.0.0-alpha"` | Default package version |
| `appportal_show_advanced` | bool | `false` | Show advanced options in AppPortal |

## Examples

```bash
# Show current config
openntx config show

# Initialize config file
openntx config init

# Set log retention to 60 days
openntx config set log_retention_days 60

# Disable notifications
openntx config set enable_notifications false

# Set default sandbox profile
openntx config set default_sandbox_profile strict

# Reset all settings to defaults
openntx config reset --yes
```

## Validation

- Unknown keys are rejected with an error message listing valid keys
- Boolean values accept: `true`, `false`, `1`, `0`, `yes`, `no`
- Sandbox profile must be one of: `strict`, `standard`, `developer`
- Numeric values must be valid integers

## Defaults

If the config file does not exist, OpenNTX uses safe defaults:
- Output directory: `dist`
- Sandbox profile: `standard`
- Notifications: enabled
- Log retention: 30 days
- Package version: `1.0.0-alpha`
- Advanced AppPortal: disabled
