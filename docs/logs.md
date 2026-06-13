# OpenNTX Logs

The `openntx logs` command manages run-plan diagnostic logs.

## List Logs

```bash
openntx logs list
openntx logs list --json
```

Lists all run-plan logs from `~/.local/state/openntx/logs/`, sorted by most recent first. Shows timestamp, app ID, app name, and status.

## Show Log

```bash
openntx logs show <path-to-log.json>
openntx logs show <app-id>
openntx logs show <app-id> --json
```

Shows a readable log summary. Accepts either:
- An exact path or filename to a log file
- An app ID (shows the latest log for that app)

## Clean Old Logs

```bash
openntx logs clean --older-than-days 30 --yes
```

Deletes old logs from the OpenNTX logs directory only. Dry-run by default; requires `--yes` to delete.

**Safety rules:**
- Never deletes files outside the logs directory
- Verifies each file is inside the canonical logs directory before deletion
- Only processes `run-plan-*.json` files

## Examples

```bash
# List all logs
openntx logs list

# Show latest log for an app
openntx logs show my-app-1234abcd

# Show a specific log file
openntx logs show ~/.local/state/openntx/logs/run-plan-my-app-1234abcd-1234567890.json

# Clean logs older than 30 days (dry-run)
openntx logs clean --older-than-days 30

# Actually delete old logs
openntx logs clean --older-than-days 30 --yes
```

## JSON Output

All list and show commands support `--json` for machine-readable output.

## Log Location

Logs are stored at:
- `$XDG_STATE_HOME/openntx/logs/` (if `XDG_STATE_HOME` is set)
- `~/.local/state/openntx/logs/` (default)
