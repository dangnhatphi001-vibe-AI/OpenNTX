#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

app_id="${2:-example-app}"
mock_xdg_data_home="$(mktemp -d)"

CARGO_BIN="${CARGO:-cargo}"
if ! command -v "${CARGO_BIN}" >/dev/null 2>&1; then
  CARGO_BIN="${HOME}/.cargo/bin/cargo"
fi

cleanup_input=""
if [[ $# -ge 1 ]]; then
  input="$1"
else
  input="$(mktemp --suffix=-setup.exe)"
  cleanup_input="${input}"
  python3 - "${input}" <<'PY'
import sys

path = sys.argv[1]
data = bytearray(2048)
data[0:2] = b"MZ"
data[0x3c:0x40] = (0x80).to_bytes(4, "little")
data[0x80:0x84] = b"PE\0\0"
data[0x84:0x86] = (0x8664).to_bytes(2, "little")
data[0x86:0x88] = (1).to_bytes(2, "little")
data[0x94:0x96] = (0x00f0).to_bytes(2, "little")
data[0x96:0x98] = (0x0002).to_bytes(2, "little")
data[0x98:0x9a] = (0x020b).to_bytes(2, "little")
data[0x98 + 16:0x98 + 20] = (0x1010).to_bytes(4, "little")
data[0x98 + 24:0x98 + 32] = (0x140000000).to_bytes(8, "little")
data[0x98 + 68:0x98 + 70] = (2).to_bytes(2, "little")
data[0x98 + 108:0x98 + 112] = (16).to_bytes(4, "little")
data[0x98 + 112 + 8:0x98 + 112 + 12] = (0x1100).to_bytes(4, "little")
data[0x98 + 112 + 12:0x98 + 112 + 16] = (40).to_bytes(4, "little")

section = 0x188
data[section:section + 8] = b".text\0\0\0"
data[section + 8:section + 12] = (0x400).to_bytes(4, "little")
data[section + 12:section + 16] = (0x1000).to_bytes(4, "little")
data[section + 16:section + 20] = (0x400).to_bytes(4, "little")
data[section + 20:section + 24] = (0x200).to_bytes(4, "little")
data[section + 36:section + 40] = (0x60000020).to_bytes(4, "little")

data[0x300 + 12:0x300 + 16] = (0x1140).to_bytes(4, "little")
data[0x300 + 16:0x300 + 20] = (0x1280).to_bytes(4, "little")
data[0x340:0x34d] = b"KERNEL32.dll\0"
with open(path, "wb") as handle:
    handle.write(data)
PY
fi

trap 'if [[ -n "${cleanup_input}" ]]; then rm -f "${cleanup_input}"; fi; rm -rf "${mock_xdg_data_home}"' EXIT

echo "OpenNTX mock install flow"
echo "Input: ${input}"
echo
echo "+ openntx analyze ${input}"
"${CARGO_BIN}" run -q -p openntx-cli -- analyze "${input}" || true
echo
echo "+ openntx manifest generate ${input}"
"${CARGO_BIN}" run -q -p openntx-cli -- manifest generate "${input}" || true
echo
echo "+ openntx install ${input}"
"${CARGO_BIN}" run -q -p openntx-cli -- install "${input}" || true
echo
echo "+ openntx install ${input} --write-plan"
XDG_DATA_HOME="${mock_xdg_data_home}" "${CARGO_BIN}" run -q -p openntx-cli -- install "${input}" --write-plan || true
registered_app_id="$(XDG_DATA_HOME="${mock_xdg_data_home}" "${CARGO_BIN}" run -q -p openntx-cli -- list | awk -F'[:|]' '/^App:/ {gsub(/^[ \t]+|[ \t]+$/, "", $2); print $2; exit}')"
echo
echo "+ openntx list"
XDG_DATA_HOME="${mock_xdg_data_home}" "${CARGO_BIN}" run -q -p openntx-cli -- list || true
if [[ -n "${registered_app_id}" ]]; then
  echo
  echo "+ openntx show ${registered_app_id}"
  XDG_DATA_HOME="${mock_xdg_data_home}" "${CARGO_BIN}" run -q -p openntx-cli -- show "${registered_app_id}" || true
  echo
  echo "+ openntx remove ${registered_app_id} --dry-run"
  XDG_DATA_HOME="${mock_xdg_data_home}" "${CARGO_BIN}" run -q -p openntx-cli -- remove "${registered_app_id}" --dry-run || true
fi
echo
echo "+ openntx package ${app_id}"
"${CARGO_BIN}" run -q -p openntx-cli -- package "${app_id}"
echo
echo "Status: demonstration only. Runtime execution and installer capture are not implemented in V0.4."
