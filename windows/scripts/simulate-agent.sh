#!/usr/bin/env bash
# Phase 5: AI Agent Waiting Gap Simulator & Live Verification Script
#
# Usage:
#   ./scripts/simulate-agent.sh <antigravity|kiro|opencode> <working|done|clear|cycle> [detail_or_seconds]
#
# Examples:
#   ./scripts/simulate-agent.sh kiro working "Refactoring auth module"
#   ./scripts/simulate-agent.sh kiro done "Kiro finished refactoring"
#   ./scripts/simulate-agent.sh opencode cycle 4
#   ./scripts/simulate-agent.sh antigravity clear

set -euo pipefail

AGENT="${1:-kiro}"
ACTION="${2:-cycle}"
ARG="${3:-}"

OVERRIDE_DIR="${HOME}/.cache/coucou"
OVERRIDE_FILE="${OVERRIDE_DIR}/agent-override.json"
mkdir -p "${OVERRIDE_DIR}"

write_state() {
  local agent_name="$1"
  local state_val="$2"
  local detail_val="$3"
  local ts
  ts="$(date +%s)"

  python3 - "$OVERRIDE_FILE" "$agent_name" "$state_val" "$detail_val" "$ts" <<'PY'
import json, os, sys

path, agent, state, detail, ts = sys.argv[1], sys.argv[2].lower(), sys.argv[3].lower(), sys.argv[4], int(sys.argv[5])
data = {}
if os.path.exists(path):
    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception:
        data = {}

if state == "clear":
    data.pop(agent, None)
else:
    data[agent] = {
        "state": state,
        "detail": detail,
        "updatedAt": ts,
    }

with open(path, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
print(json.dumps({"ok": True, "agent": agent, "state": state, "detail": detail, "overrideFile": path}))
PY
}

if [[ "$ACTION" == "cycle" ]]; then
  WAIT_SECS="${ARG:-3}"
  write_state "$AGENT" "working" "${AGENT^} agent executing task..."
  sleep "$WAIT_SECS"
  write_state "$AGENT" "done" "${AGENT^} agent done, review it!"
else
  DETAIL="${ARG:-${AGENT^} agent ${ACTION}}"
  write_state "$AGENT" "$ACTION" "$DETAIL"
fi
