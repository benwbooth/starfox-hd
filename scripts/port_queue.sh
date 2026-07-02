#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
queue_file="${PORT_QUEUE_FILE:-$repo_root/automation/port_queue.tsv}"
lock_file="${PORT_QUEUE_LOCK_FILE:-$repo_root/automation/port_queue.lock}"

usage() {
    cat <<'EOF'
Usage:
  scripts/port_queue.sh list [lane]
  scripts/port_queue.sh ready [lane]
  scripts/port_queue.sh next [lane]
  scripts/port_queue.sh claim-next [lane]
  scripts/port_queue.sh claim-task <task-id>
  scripts/port_queue.sh show <task-id>
  scripts/port_queue.sh set-status <task-id> <pending|in_progress|done|blocked|failed>
  scripts/port_queue.sh requeue-in-progress
  scripts/port_queue.sh stats [lane]
EOF
}

die() {
    printf '%s\n' "$*" >&2
    exit 1
}

require_queue() {
    [[ -f "$queue_file" ]] || die "Queue file not found: $queue_file"
    mkdir -p "$(dirname "$lock_file")"
}

require_flock() {
    command -v flock >/dev/null 2>&1 || die "flock is required"
}

validate_status() {
    case "$1" in
        pending|in_progress|done|blocked|failed) ;;
        *) die "Invalid status: $1" ;;
    esac
}

queue_awk() {
    local lane_filter="${1:-}"
    awk -F '\t' -v lane_filter="$lane_filter" '
        NR == FNR {
            if (NR > 1) {
                task_status[$1] = $2
                task_lane[$1] = $11
                if ($2 == "in_progress" && $11 != "") {
                    busy_lane[$11] = 1
                }
            }
            next
        }
        NR == 1 { next }
        {
            lane = $11
            if (lane_filter != "" && lane != lane_filter) {
                next
            }
            row_ready = 0
            if ($2 == "pending") {
                deps = $9
                ready = 1
                if (lane != "" && busy_lane[lane]) {
                    ready = 0
                }
                if (ready && deps != "" && deps != "-") {
                    dep_count = split(deps, dep_list, ",")
                    for (i = 1; i <= dep_count; i++) {
                        dep = dep_list[i]
                        gsub(/^[[:space:]]+|[[:space:]]+$/, "", dep)
                        if (dep == "" || dep == "-") {
                            continue
                        }
                        if (!(dep in task_status) || task_status[dep] != "done") {
                            ready = 0
                            break
                        }
                    }
                }
                row_ready = ready
            }
            print $0 "\t" row_ready
        }
    ' "$queue_file" "$queue_file"
}

set_status_locked() {
    local task_id="$1"
    local new_status="$2"
    local tmp

    tmp="$(mktemp)"
    awk -F '\t' -v OFS='\t' -v task_id="$task_id" -v new_status="$new_status" '
        NR == 1 { print; next }
        $1 == task_id { $2 = new_status; found = 1 }
        { print }
        END { exit(found ? 0 : 1) }
    ' "$queue_file" >"$tmp" || {
        rm -f "$tmp"
        die "Task not found: $task_id"
    }

    mv "$tmp" "$queue_file"
}

list_cmd() {
    local lane_filter="${1:-}"
    require_queue
    awk -F '\t' -v lane_filter="$lane_filter" '
        NR == 1 { next }
        lane_filter != "" && $11 != lane_filter { next }
        {
            printf "%-18s %-12s %-3s %-10s %-9s %s\n", $1, $2, $3, $4, $11, $10
        }
    ' "$queue_file"
}

ready_rows() {
    local lane_filter="${1:-}"
    require_queue
    queue_awk "$lane_filter" | awk -F '\t' '$12 == "1" { print }'
}

ready_cmd() {
    local lane_filter="${1:-}"
    ready_rows "$lane_filter" | awk -F '\t' '
        {
            printf "%-18s %-12s %-3s %-10s %-9s %s\n", $1, $2, $3, $4, $11, $10
        }
    '
}

next_cmd() {
    local lane_filter="${1:-}"
    ready_rows "$lane_filter" | head -n1 | cut -f1-11
}

claim_next_cmd() {
    local lane_filter="${1:-}"
    local row
    local id

    require_queue
    require_flock

    exec 9>"$lock_file"
    flock 9

    row="$(ready_rows "$lane_filter" | head -n1 | cut -f1-11)"
    if [[ -z "$row" ]]; then
        flock -u 9
        exec 9>&-
        return 0
    fi

    IFS=$'\t' read -r id _ <<<"$row"
    set_status_locked "$id" in_progress

    printf '%s\n' "$row"

    flock -u 9
    exec 9>&-
}

claim_task_cmd() {
    local task_id="$1"
    local row
    local rc

    require_queue
    require_flock

    exec 9>"$lock_file"
    flock 9

    if row="$(awk -F '\t' -v task_id="$task_id" '
        NR == FNR {
            if (NR > 1) {
                task_status[$1] = $2
                task_lane[$1] = $11
                if ($2 == "in_progress" && $11 != "") {
                    busy_lane[$11] = 1
                }
            }
            next
        }
        NR == 1 { next }
        $1 != task_id { next }
        {
            found = 1
            lane = $11
            if ($2 != "pending") {
                exit 2
            }
            if (lane != "" && busy_lane[lane]) {
                exit 3
            }
            deps = $9
            if (deps != "" && deps != "-") {
                dep_count = split(deps, dep_list, ",")
                for (i = 1; i <= dep_count; i++) {
                    dep = dep_list[i]
                    gsub(/^[[:space:]]+|[[:space:]]+$/, "", dep)
                    if (dep == "" || dep == "-") {
                        continue
                    }
                    if (!(dep in task_status) || task_status[dep] != "done") {
                        exit 4
                    }
                }
            }
            print
        }
        END {
            if (!found) {
                exit 5
            }
        }
    ' "$queue_file" "$queue_file")"; then
        rc=0
    else
        rc=$?
    fi

    case "$rc" in
        0)
            set_status_locked "$task_id" in_progress
            printf '%s\n' "$row"
            ;;
        2)
            die "Task is not pending: $task_id"
            ;;
        3)
            die "Task lane already busy: $task_id"
            ;;
        4)
            die "Task dependencies are not done: $task_id"
            ;;
        5)
            die "Task not found: $task_id"
            ;;
        *)
            die "Could not claim task: $task_id"
            ;;
    esac

    flock -u 9
    exec 9>&-
}

show_cmd() {
    local task_id="$1"
    require_queue
    awk -F '\t' -v task_id="$task_id" '
        NR == 1 { print; next }
        $1 == task_id { print; found = 1 }
        END { exit(found ? 0 : 1) }
    ' "$queue_file" || die "Task not found: $task_id"
}

set_status_cmd() {
    local task_id="$1"
    local new_status="$2"

    validate_status "$new_status"
    require_queue
    require_flock

    exec 9>"$lock_file"
    flock 9
    set_status_locked "$task_id" "$new_status"
    flock -u 9
    exec 9>&-
}

requeue_in_progress_cmd() {
    local tmp

    require_queue
    require_flock

    exec 9>"$lock_file"
    flock 9

    tmp="$(mktemp)"
    awk -F '\t' -v OFS='\t' '
        NR == 1 { print; next }
        $2 == "in_progress" { $2 = "pending" }
        { print }
    ' "$queue_file" >"$tmp"

    mv "$tmp" "$queue_file"

    flock -u 9
    exec 9>&-
}

stats_cmd() {
    local lane_filter="${1:-}"
    require_queue
    awk -F '\t' -v lane_filter="$lane_filter" '
        NR == FNR {
            if (NR > 1) {
                task_status[$1] = $2
                task_lane[$1] = $11
                if ($2 == "in_progress" && $11 != "") {
                    busy_lane[$11] = 1
                }
            }
            next
        }
        NR == 1 { next }
        {
            lane = $11
            if (lane_filter != "" && lane != lane_filter) {
                next
            }
            count[$2]++
            if ($2 == "pending") {
                ready = 1
                if (lane != "" && busy_lane[lane]) {
                    ready = 0
                }
                deps = $9
                if (ready && deps != "" && deps != "-") {
                    dep_count = split(deps, dep_list, ",")
                    for (i = 1; i <= dep_count; i++) {
                        dep = dep_list[i]
                        gsub(/^[[:space:]]+|[[:space:]]+$/, "", dep)
                        if (dep == "" || dep == "-") {
                            continue
                        }
                        if (!(dep in task_status) || task_status[dep] != "done") {
                            ready = 0
                            break
                        }
                    }
                }
                if (ready) {
                    ready_count++
                }
            }
        }
        END {
            printf "pending=%d\n", count["pending"] + 0
            printf "ready=%d\n", ready_count + 0
            printf "in_progress=%d\n", count["in_progress"] + 0
            printf "done=%d\n", count["done"] + 0
            printf "blocked=%d\n", count["blocked"] + 0
            printf "failed=%d\n", count["failed"] + 0
        }
    ' "$queue_file" "$queue_file"
}

cmd="${1:-}"
case "$cmd" in
    list)
        [[ $# -le 2 ]] || usage
        list_cmd "${2:-}"
        ;;
    ready)
        [[ $# -le 2 ]] || usage
        ready_cmd "${2:-}"
        ;;
    next)
        [[ $# -le 2 ]] || usage
        next_cmd "${2:-}"
        ;;
    claim-next)
        [[ $# -le 2 ]] || usage
        claim_next_cmd "${2:-}"
        ;;
    claim-task)
        [[ $# -eq 2 ]] || usage
        claim_task_cmd "$2"
        ;;
    show)
        [[ $# -eq 2 ]] || usage
        show_cmd "$2"
        ;;
    set-status)
        [[ $# -eq 3 ]] || usage
        set_status_cmd "$2" "$3"
        ;;
    requeue-in-progress)
        [[ $# -eq 1 ]] || usage
        requeue_in_progress_cmd
        ;;
    stats)
        [[ $# -le 2 ]] || usage
        stats_cmd "${2:-}"
        ;;
    *)
        usage
        exit 1
        ;;
esac
