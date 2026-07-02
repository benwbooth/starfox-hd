#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
queue_file="${PORT_QUEUE_FILE:-$repo_root/automation/port_queue.tsv}"
schema_file="$repo_root/automation/port_result.schema.json"
log_dir="$repo_root/automation/logs"
mkdir -p "$log_dir"

dry_run=0
task_id=""
count=1
model=""
lane=""

usage() {
    cat <<'EOF'
Usage:
  scripts/port_loop.sh [--dry-run] [--task <task-id>] [--lane <lane>] [--count N] [--model MODEL]

Behavior:
  - Atomically claims the next runnable task unless --task is set.
  - Runs codex exec non-interactively on that bounded task.
  - Runs scripts/port_validate.sh.
  - Marks the task done on success, failed on error.
EOF
}

die() {
    printf '%s\n' "$*" >&2
    exit 1
}

json_bool() {
    local file="$1"
    local key="$2"
    if command -v jq >/dev/null 2>&1; then
        jq -r ".$key" "$file"
    else
        sed -n "s/.*\"$key\"[[:space:]]*:[[:space:]]*\\(true\\|false\\).*/\\1/p" "$file" | head -n1
    fi
}

json_string() {
    local file="$1"
    local key="$2"
    if command -v jq >/dev/null 2>&1; then
        jq -r ".$key" "$file"
    else
        sed -n "s/.*\"$key\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" "$file" | head -n1
    fi
}

get_task_row() {
    if [[ "$dry_run" -eq 1 ]]; then
        if [[ -n "$task_id" ]]; then
            "$repo_root/scripts/port_queue.sh" show "$task_id" | awk -F '\t' 'NR == 2 { print }'
        else
            "$repo_root/scripts/port_queue.sh" next "$lane"
        fi
        return
    fi

    if [[ -n "$task_id" ]]; then
        "$repo_root/scripts/port_queue.sh" claim-task "$task_id"
    else
        "$repo_root/scripts/port_queue.sh" claim-next "$lane"
    fi
}

build_prompt() {
    local id="$1"
    local kind="$2"
    local asm_file="$3"
    local start_ref="$4"
    local end_ref="$5"
    local target_file="$6"
    local depends_on="$7"
    local description="$8"

    cat <<EOF
You are working in $repo_root.

Task id: $id
Task kind: $kind
Source ASM: $asm_file
Bounds: $start_ref -> $end_ref
Target file: $target_file
Depends on: $depends_on
Description: $description

Mandatory rules:
- Port this slice literally from assembly to C first.
- Do not revive Python or generated headers as active source-of-truth.
- Keep changes bounded to this slice and strictly required local helpers.
- Preserve original routine and macro intent as closely as flat-memory C allows.
- No silent fallbacks. Missing cases must stay explicit.
- Run scripts/port_validate.sh before finishing.

When you finish, return JSON that matches $schema_file.
Set "task_id" to "$id".
Set "completed" to true only if the bounded slice is actually ported and validation passed.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            dry_run=1
            shift
            ;;
        --task)
            [[ $# -ge 2 ]] || die "--task requires an argument"
            task_id="$2"
            shift 2
            ;;
        --count)
            [[ $# -ge 2 ]] || die "--count requires an argument"
            count="$2"
            shift 2
            ;;
        --lane)
            [[ $# -ge 2 ]] || die "--lane requires an argument"
            lane="$2"
            shift 2
            ;;
        --model)
            [[ $# -ge 2 ]] || die "--model requires an argument"
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

[[ -f "$queue_file" ]] || die "Queue file not found: $queue_file"
[[ -f "$schema_file" ]] || die "Schema file not found: $schema_file"

for ((i = 0; i < count; i++)); do
    row="$(get_task_row)"
    [[ -n "$row" ]] || {
        printf 'PORT LOOP: no pending tasks\n'
        exit 0
    }

    IFS=$'\t' read -r id status priority kind asm_file start_ref end_ref target_file depends_on description owner_lane <<<"$row"
    prompt_file="$(mktemp)"
    result_file="$(mktemp)"
    timestamp="$(date +%Y%m%d-%H%M%S)"
    log_prefix="$log_dir/${timestamp}-${id}"

    build_prompt "$id" "$kind" "$asm_file" "$start_ref" "$end_ref" "$target_file" "$depends_on" "$description" >"$prompt_file"

    if [[ "$dry_run" -eq 1 ]]; then
        printf 'PORT LOOP DRY RUN\n'
        printf 'task=%s status=%s priority=%s kind=%s lane=%s\n' "$id" "$status" "$priority" "$kind" "${owner_lane:-}"
        cat "$prompt_file"
        rm -f "$prompt_file" "$result_file"
        exit 0
    fi

    codex_cmd=(codex exec --full-auto -C "$repo_root" --output-schema "$schema_file" -o "$result_file")
    if [[ -n "$model" ]]; then
        codex_cmd+=(--model "$model")
    fi
    codex_cmd+=(-)

    if ! "${codex_cmd[@]}" <"$prompt_file"; then
        cp "$prompt_file" "${log_prefix}.prompt.txt"
        [[ -f "$result_file" ]] && cp "$result_file" "${log_prefix}.result.json" || true
        "$repo_root/scripts/port_queue.sh" set-status "$id" failed
        die "Codex exec failed for task $id"
    fi

    cp "$prompt_file" "${log_prefix}.prompt.txt"
    cp "$result_file" "${log_prefix}.result.json"

    completed="$(json_bool "$result_file" completed)"
    reported_id="$(json_string "$result_file" task_id)"
    [[ "$reported_id" == "$id" ]] || {
        "$repo_root/scripts/port_queue.sh" set-status "$id" failed
        die "Task id mismatch in result: expected $id got ${reported_id:-<empty>}"
    }

    if ! "$repo_root/scripts/port_validate.sh"; then
        "$repo_root/scripts/port_queue.sh" set-status "$id" failed
        die "Validation failed for task $id"
    fi

    if [[ "$completed" != "true" ]]; then
        "$repo_root/scripts/port_queue.sh" set-status "$id" failed
        die "Task $id was not reported complete"
    fi

    "$repo_root/scripts/port_queue.sh" set-status "$id" done
    printf 'PORT LOOP: completed %s\n' "$id"

    rm -f "$prompt_file" "$result_file"
    if [[ -n "$task_id" ]]; then
        break
    fi
done
