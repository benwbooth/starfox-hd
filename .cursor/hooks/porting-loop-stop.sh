#!/usr/bin/env bash
# Cursor `stop` hook: keep the Star Fox HD leaf-first porting loop alive.
#
# Enable:  touch .cursor/porting-loop-on
# Disable: rm .cursor/porting-loop-on   (or say "stop the porting loop")
#
# Reads JSON on stdin from Cursor. When enabled and the turn completed cleanly,
# emits followup_message so Cursor auto-submits the next porting tick.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ENABLE="$ROOT/.cursor/porting-loop-on"
SKIP="$ROOT/.cursor/skip-porting-loop"

noop() {
  printf '%s\n' '{}'
  exit 0
}

input="$(cat || true)"

# Escape hatch for one turn (self-clears).
if [[ -f "$SKIP" ]]; then
  rm -f "$SKIP"
  noop
fi

# Must be explicitly armed.
if [[ ! -f "$ENABLE" ]]; then
  noop
fi

status="$(printf '%s' "$input" | python3 -c 'import json,sys
try:
  d=json.load(sys.stdin)
except Exception:
  d={}
print(d.get("status") or "")' 2>/dev/null || true)"

loop_count="$(printf '%s' "$input" | python3 -c 'import json,sys
try:
  d=json.load(sys.stdin)
except Exception:
  d={}
print(d.get("loop_count") if d.get("loop_count") is not None else 0)' 2>/dev/null || true)"

# Only continue after a clean agent completion (not abort / error).
if [[ "$status" != "completed" ]]; then
  noop
fi

# Soft cap inside the script (hooks.json also has loop_limit).
if [[ "${loop_count:-0}" =~ ^[0-9]+$ ]] && (( loop_count >= 500 )); then
  noop
fi

# Latest session breadcrumb for continuity.
summary=""
if [[ -f "$ROOT/re_loop_sessions/2026-07-09.txt" ]]; then
  summary="$(tail -n 12 "$ROOT/re_loop_sessions/2026-07-09.txt" | tr '\n' ' ' | sed 's/  */ /g' | cut -c1-400)"
fi

msg=$(python3 - <<'PY' "$loop_count" "$summary"
import json, sys
loop = sys.argv[1]
summary = sys.argv[2]
body = (
  "Continue leaf-first ROM→Rust port/verify from docs/PORTING_LOOP.md "
  f"(stop-hook tick loop_count={loop}). "
  "Pick the next unchecked enemy_boss_strat / weapon / explode leaf, "
  "port or verify with tests/sf-oracle, update docs/function_ledger.tsv + "
  "docs/PORTING_LOOP.md + re_loop_sessions, then end the turn so the stop "
  "hook can re-arm. Meaningful names/structs; no segmented 16-bit model. "
  "Aim 100%. To stop: delete .cursor/porting-loop-on or say stop the porting loop."
)
if summary:
  body += f" Recent session log: {summary}"
print(json.dumps({"followup_message": body}))
PY
)

printf '%s\n' "$msg"
exit 0
