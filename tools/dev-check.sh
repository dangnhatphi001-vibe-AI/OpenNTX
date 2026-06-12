#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

CARGO_BIN="${CARGO:-cargo}"
if ! command -v "${CARGO_BIN}" >/dev/null 2>&1; then
  CARGO_BIN="${HOME}/.cargo/bin/cargo"
fi

"${CARGO_BIN}" fmt --all -- --check
"${CARGO_BIN}" test --workspace

python3 - <<'PY'
import json
from pathlib import Path

roots = [Path("schemas"), Path("examples")]
for root in roots:
    for path in sorted(root.rglob("*.json")):
        with path.open("r", encoding="utf-8") as handle:
            json.load(handle)
        print(f"json ok: {path}")
PY

python3 - <<'PY'
from pathlib import Path
import json
import sys

try:
    import jsonschema
except Exception:
    print("jsonschema not installed; skipping schema validation")
    sys.exit(0)

pairs = [
    ("schemas/app-manifest.schema.json", "examples/manifests/portable-app.openntx.json"),
    ("schemas/app-manifest.schema.json", "examples/manifests/captured-installer.openntx.json"),
    ("schemas/capture-report.schema.json", "examples/capture-reports/example-capture-report.json"),
]

for schema_path, data_path in pairs:
    schema = json.loads(Path(schema_path).read_text(encoding="utf-8"))
    data = json.loads(Path(data_path).read_text(encoding="utf-8"))
    jsonschema.validate(instance=data, schema=schema)
    print(f"schema ok: {data_path}")
PY
