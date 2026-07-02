#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
queue_file="${PORT_QUEUE_FILE:-$repo_root/automation/port_queue.tsv}"

fail() {
    printf 'VALIDATE: %s\n' "$*" >&2
    exit 1
}

ok() {
    printf 'VALIDATE: %s\n' "$*"
}

cd "$repo_root"

[[ -f "$queue_file" ]] || fail "missing queue file: $queue_file"

awk -F '\t' '
    NR == 1 {
        if ($11 != "owner_lane") {
            printf "missing owner_lane column in queue header\n" > "/dev/stderr"
            exit 1
        }
        next
    }
    {
        ids[$1] = 1
        if (seen[$1]++) {
            printf "duplicate task id: %s\n", $1 > "/dev/stderr"
            exit 1
        }
        if ($2 !~ /^(pending|in_progress|done|blocked|failed)$/) {
            printf "invalid status for %s: %s\n", $1, $2 > "/dev/stderr"
            exit 1
        }
        if ($11 == "") {
            printf "missing owner_lane for %s\n", $1 > "/dev/stderr"
            exit 1
        }
        if ($2 == "in_progress" && in_progress_lane[$11]++) {
            printf "multiple in_progress rows in lane %s\n", $11 > "/dev/stderr"
            exit 1
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
                referenced[NR, dep] = $1
            }
        }
    }
    END {
        for (key in referenced) {
            split(key, parts, SUBSEP)
            dep = parts[2]
            owner = referenced[key]
            if (!(dep in ids)) {
                printf "unknown dependency for %s: %s\n", owner, dep > "/dev/stderr"
                exit 1
            }
        }
    }
' "$queue_file" || fail "queue integrity check failed"
ok "queue integrity"

generated_refs="$(rg -n '#include ".*(paths_data|istrat_shapes_data|level[0-9a-z_]*_data)\.h"' \
    src CMakeLists.txt || true)"
if [[ -n "$generated_refs" ]]; then
    printf '%s\n' "$generated_refs" >&2
    fail "active source still includes generated data headers"
fi
ok "no generated data header includes in active sources"

nix develop --command cmake --build build -j4 >/tmp/starfox-port-validate-build.log 2>&1 || {
    cat /tmp/starfox-port-validate-build.log >&2
    fail "build failed"
}
ok "build"

ok "validation passed"
