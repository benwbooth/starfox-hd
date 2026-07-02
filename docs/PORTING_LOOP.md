# Porting Loop

This repo now has a bounded queue-driven Codex loop for the literal ASM->C phase.

It is not an endless background agent. It is a controlled driver around one task at a time.

The queue is dependency-aware and lane-aware. A row is only claimable when:

- its prerequisites are already `done`
- its lane has no other `in_progress` row

## Files

- `automation/port_queue.tsv`
  - Ordered task queue.
  - Includes an `owner_lane` column for safe parallel workers.
- `automation/port_result.schema.json`
  - Expected final JSON from `codex exec`.
- `scripts/port_queue.sh`
  - Queue inspection, atomic claims, and status updates.
- `scripts/port_validate.sh`
  - Mechanical validation gate.
- `scripts/port_loop.sh`
  - Bounded driver that atomically claims one task and runs Codex on it.
- `scripts/port_workers.sh`
  - Parallel worker-pool wrapper that runs one worker per lane.
- `docs/PHASE1_CHECKLIST.md`
  - Human-readable phase-1 file/section checklist. The queue is the executable subset.

## Status Model

- `pending`
- `in_progress`
- `done`
- `blocked`
- `failed`

## Validation Gate

`scripts/port_validate.sh` currently enforces:

- queue integrity
- one `in_progress` row per lane
- no active generated-header includes
- clean compile

It does not claim gameplay correctness. It is a mechanical gate only.

## Queue Commands

List all tasks:

```bash
scripts/port_queue.sh list
```

List tasks for one lane:

```bash
scripts/port_queue.sh list map
```

List tasks that are actually claimable right now:

```bash
scripts/port_queue.sh ready
```

List claimable tasks for one lane:

```bash
scripts/port_queue.sh ready strat
```

Show queue counts:

```bash
scripts/port_queue.sh stats
```

Show queue counts for one lane:

```bash
scripts/port_queue.sh stats path
```

Show one task:

```bash
scripts/port_queue.sh show sf-boss7-shapes
```

Atomically claim the next task:

```bash
scripts/port_queue.sh claim-next
```

Atomically claim the next task in one lane:

```bash
scripts/port_queue.sh claim-next map
```

Atomically claim one specific task:

```bash
scripts/port_queue.sh claim-task sf-boss7-shapes
```

Set one task status:

```bash
scripts/port_queue.sh set-status sf-boss7-shapes pending
```

Reset stale `in_progress` rows after an aborted run:

```bash
scripts/port_queue.sh requeue-in-progress
```

## Loop Commands

Dry run the next claimable task:

```bash
scripts/port_loop.sh --dry-run
```

Run the next claimable task:

```bash
scripts/port_loop.sh
```

Run the next claimable task in one lane:

```bash
scripts/port_loop.sh --lane path
```

Run a specific task:

```bash
scripts/port_loop.sh --task sf-boss7-shapes
```

Run multiple queued tasks in sequence:

```bash
scripts/port_loop.sh --count 3
```

Run one worker per active lane until the queue drains:

```bash
scripts/port_workers.sh
```

Run only selected lanes:

```bash
scripts/port_workers.sh --lane map --lane path --lane strat
```

## Rules the Loop Enforces

- one bounded ASM slice per task
- dependencies must already be `done` before a task is considered runnable
- queue claims are atomic via `flock`
- only one `in_progress` row is allowed per lane
- unattended runs use `codex exec -s workspace-write -a never`
- no Python-generated runtime source-of-truth
- literal ASM->C first
- validation after each task

## Recommended Automation Mode

For unattended phase-1 work, the default should be:

```bash
scripts/port_workers.sh
```

That gives you:

- atomic queue claims
- one worker per lane
- failure propagation back to the parent supervisor
- resumable queue state on disk
- validation after every claimed row

Use `scripts/port_loop.sh` directly only when you want to force one specific row or one specific lane.

## Important Limitation

The loop is only as good as the queue granularity.

If a task is too broad, the loop becomes sloppy. Keep each row narrow:

- one map slice
- one path slice
- one strategy slice
- one shape-support slice

Do not use “support” as a grab-bag bucket. Prefer real file/section rows such as:

- `LEVEL1_1.ASM:L69-L74`
- `PATHDATA.ASM:chase7_1 -> chase7_2`
- `GBSTRATS.ASM:boss1_Istrat`
