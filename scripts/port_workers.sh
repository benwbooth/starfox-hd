#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log_dir="$repo_root/automation/logs/workers"
mkdir -p "$log_dir"

poll_seconds=5
model=""
lanes=()

usage() {
    cat <<'EOF'
Usage:
  scripts/port_workers.sh [--lane <lane>]... [--poll-seconds N] [--model MODEL]

Behavior:
  - Starts one background worker loop per lane.
  - Each worker claims only its own lane.
  - Workers keep polling until their lane has no pending, ready, or in_progress tasks left.
  - The script exits non-zero and stops the remaining workers if any lane worker fails.

Examples:
  scripts/port_workers.sh
  scripts/port_workers.sh --lane map --lane path --lane strat
EOF
}

parse_stat() {
    local key="$1"
    awk -F '=' -v key="$key" '$1 == key { print $2 }'
}

default_lanes() {
    awk -F '\t' '
        NR == 1 { next }
        $11 != "" && $2 != "done" && $2 != "blocked" && !seen[$11]++ { print $11 }
    ' "$repo_root/automation/port_queue.tsv"
}

lane_worker() {
    local lane="$1"
    local stats
    local pending
    local ready
    local in_progress
    local loop_cmd

    while true; do
        stats="$("$repo_root/scripts/port_queue.sh" stats "$lane")"
        pending="$(printf '%s\n' "$stats" | parse_stat pending)"
        ready="$(printf '%s\n' "$stats" | parse_stat ready)"
        in_progress="$(printf '%s\n' "$stats" | parse_stat in_progress)"

        if [[ "${pending:-0}" -eq 0 && "${ready:-0}" -eq 0 && "${in_progress:-0}" -eq 0 ]]; then
            printf 'WORKER[%s]: lane complete\n' "$lane"
            return 0
        fi

        if [[ "${ready:-0}" -eq 0 ]]; then
            sleep "$poll_seconds"
            continue
        fi

        loop_cmd=("$repo_root/scripts/port_loop.sh" "--lane" "$lane" "--count" "1")
        if [[ -n "$model" ]]; then
            loop_cmd+=("--model" "$model")
        fi
        "${loop_cmd[@]}"
    done
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --lane)
            [[ $# -ge 2 ]] || {
                usage
                exit 1
            }
            lanes+=("$2")
            shift 2
            ;;
        --poll-seconds)
            [[ $# -ge 2 ]] || {
                usage
                exit 1
            }
            poll_seconds="$2"
            shift 2
            ;;
        --model)
            [[ $# -ge 2 ]] || {
                usage
                exit 1
            }
            model="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage
            exit 1
            ;;
    esac
done

if [[ ${#lanes[@]} -eq 0 ]]; then
    while IFS= read -r lane; do
        [[ -n "$lane" ]] || continue
        lanes+=("$lane")
    done < <(default_lanes)
fi

[[ ${#lanes[@]} -gt 0 ]] || {
    printf 'PORT WORKERS: no active lanes\n'
    exit 0
}

pids=()

cleanup() {
    local pid
    for pid in "${pids[@]:-}"; do
        kill "$pid" 2>/dev/null || true
    done
}

trap cleanup INT TERM

for lane in "${lanes[@]}"; do
    (
        lane_worker "$lane"
    ) >"$log_dir/${lane}.log" 2>&1 &
    pids+=("$!")
done

status=0
remaining=("${pids[@]}")

while [[ ${#remaining[@]} -gt 0 ]]; do
    if ! wait -n; then
        status=1
        cleanup
        wait || true
        break
    fi

    new_remaining=()
    for pid in "${remaining[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            new_remaining+=("$pid")
        fi
    done
    remaining=("${new_remaining[@]}")
done

exit "$status"
