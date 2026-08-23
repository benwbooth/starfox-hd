# Lossless ROM reconstruction and automated parity

## Decision

The canonical roundtrip is **retail ROM -> reviewed assembly -> rebuilt ROM**,
not retail ROM -> ordinary C -> compiler output. C may be emitted as a readable
semantic view or used for an isolated experiment, but it is not a proof format.

A 65C816 C compiler is free to change instruction selection, status-flag use,
stack traffic, bank placement, branch widths, interrupt boundaries, and cycle
timing. The Super FX programs are a second instruction set with their own
pipeline and delay-slot behavior. Consequently, correct C can produce a ROM
that is functionally close while being observably different on the original
hardware.

The shipping Rust port remains a modern, flat-memory implementation with typed
domain structs. Source-machine registers, banks, and byte-addressed memory are
permitted only in this verification toolchain and `sf-oracle`.

## Exact rebuild ladder

`tools/rom_roundtrip.py` starts with a lossless image in which unreviewed ranges
come directly from the hash-bound retail ROM. A manifest can then promote one
range at a time to tracked WLA-DX assembly. The link succeeds only if the final
image has the expected size, and the command succeeds only if every rebuilt
byte equals the retail input.

This produces a monotonic proof:

1. **Byte-backed** — exact by construction, not yet reviewed.
2. **Instruction-backed** — disassembled, reviewed, and reassembled to the
   exact retail bytes.
3. **Semantically lifted** — the instruction-backed routine has a typed,
   processor-independent contract and generated differential vectors.
4. **Native Rust certified** — the shipping implementation matches the retail
   routine over exhaustive inputs where finite, otherwise boundary sets plus
   coverage-guided randomized inputs.
5. **Integrated certified** — deterministic full-machine scenarios have no
   unexplained divergence in owned state, video, audio, timing events, or
   coverage.

The SF1 manifest already promotes the complete retail runtime RNG, including
its long-call entry, to real 65C816 assembly. The same routine is compared
directly with the native Rust RNG by `sf-oracle` over multiple exact state
sequences. The SF2 manifest establishes the hash-bound baseline; its first
promoted CPU and Super FX regions are the next reconstruction work.

Run both exact rebuilds inside the project development environment:

```text
nix develop --command ./scripts/verify_rom_roundtrip.sh
```

The ROMs are user-owned and gitignored. Generated objects, symbols, and rebuilt
ROMs are temporary unless `tools/rom_roundtrip.py` is given `--work-dir` or
`--output`.

## Replacement and differential loop

Byte identity is the acceptance condition while recovering assembly. It is not
required after deliberately replacing a routine with a semantic lift. At that
point the rebuilt test ROM keeps the original address and ABI entry but routes
the selected routine to generated replacement code in reserved cartridge
space. The original and replacement ROMs then receive identical inputs from
the same save state.

The automated comparator records the first divergence at these layers:

- 65C816 and Super FX instruction boundaries for oracle diagnosis;
- WRAM and cartridge-RAM writes, grouped by named state ownership;
- DMA, PPU, controller, interrupt, and audio-port events with timing;
- native-resolution frame pixels and audio sample blocks;
- native Rust domain snapshots mapped to the same semantic fields.

The fast in-process `sf-oracle` machine is used for routine and frame-level
minimization. Mesen remains the independent full-system check for scenarios
whose correctness depends on hardware behavior not completely modeled by the
in-process oracle.

## Semantic first-divergence traces

`sf-difftest` now has a versioned JSON-lines format shared by retail-oracle and
native-port adapters. Each line is one deterministic frame containing:

- a monotonically increasing sequence, source frame, and controller input;
- named scalar game fields;
- typed objects aligned by stable semantic identity rather than storage slot;
- ordered gameplay events;
- optional video and audio item counts and hashes.

Field names describe game concepts such as `view.forward_velocity` and
`position.z`. The retail adapter may read those values from source storage, but
the trace contains no source addresses or processor state and the native
adapter reads ordinary Rust struct fields. Object ordering therefore cannot
create a false mismatch, while duplicate identities and non-monotonic traces
are rejected.

Compare two saved traces with:

```text
nix develop --command bash -c \
  "cd rust && cargo run -p sf-difftest -- --semantic retail.jsonl native.jsonl"
```

The comparator aligns frames by sequence and reports the earliest exact path,
for example `objects["primary-enemy"].fields.position.z`. It compares frame
metadata, scalar fields, objects, ordered events, video, and audio in a fixed
order so a failure is reproducible and immediately points to the owning native
subsystem.

`sf-oracle/tests/semantic_trace.rs` is the first live adapter proof. It runs the
retail game's named straight-motion strategy and the flat native Rust strategy
for 30 frames, then compares position, velocity, and view motion through this
shared format. The same test target now boots the complete retail cartridge and
the native shell under one physical controller trace through the first gameplay
handoff. All 18 retained phase records match exactly: Boot plus 17 transitions.
Attract begins at sampled tick 43; title, controller layout, destination
selection, planet-map setup, interactive route selection, ship flash, map fade,
planet isolation, recenter, Pepper preparation, planet zoom, heading reveal,
briefing, dismissal, exit fade, and gameplay then occur at relative ticks 86,
248, 318, 393, 416, 459, 487, 498, 500, 511, 515, 582, 611, 798, 805, and 816.
The same test locks retail
briefing-cursor values 0, 1, 2, 64, and 103 at global ticks 654, 656, 657, 761,
and 839.

The strict coexecution currently continues through global tick 1063, the last
frame before the first unresolved post-launch map-state divergence. From the
first initialized gameplay frame onward it compares
the background, game frame, player forward velocity and depth delta, active
object slots, flat shape identities, every object position, and the launch
wingmates' departure counters. First-divergence work corrected the opening
camera's pre-chase carry branch, source active-list ordering, player and path
initializer fallthrough, the launch player's view-speed chase, boost-flame
scheduling, the exact wingmate removal boundary, the two-stage scramble wipe,
and the exact wingmate-removal boundary. Later ticks are intentionally not
certified: the former adapter copied retail state and resynchronized randomness
across known divergence windows, which could make an inaccurate port pass.
Strict scenarios now fail instead of substituting, normalizing, quarantining,
or resynchronizing native state.

The shipping correction uses ordinary typed counters, semantic phases, and a
semantic fade-rate enum. It preserves the source's 40 zoom steps while
presenting their measured 67-tick workload, the two terminating heading passes,
the fast and settled Pepper text cadences, the full 0-through-255 source cursor,
and the dismissal/fade handoffs. Original instruction-entry watches, source
addresses, and processor state remain confined to the retail oracle.

The longer diagnostic remains executable with:

```text
nix develop --command bash -c \
  "cd rust && cargo run -p sf-oracle --example sf1_frontend_diff -- 880"
```

It now exits successfully after selecting GAME, completing the controller fade,
building the initial planet map, confirming Route 1, presenting the selected
planet and General Pepper briefing, and entering gameplay. `sf1_state_timeline`
remains the lower-level diagnostic. Cart-derived observables prove that the
controller layout becomes interactive at global tick 297, destination selection
at 361, GAME at 380, the controller fade ends at 436, route selection becomes
interactive at 459, and the held confirmation is consumed at 502. The native
port retains a fixed 20 Hz simulation and represents the measured boundaries
with typed presentation state rather than recreating processor workload. The
remaining work must extend the adapter from the post-launch fade through the
rest of gameplay, endings, video/audio hashes, and source-edge coverage before
any whole-game parity claim.

## Coverage closure

No finite test suite alone proves every game feature. The project may claim
full parity only when all of these mechanically checked gates are closed:

- every reachable ROM instruction belongs to a decoded code region or an
  explicitly classified data region;
- every reachable branch edge and indirect dispatch target is exercised by at
  least one retained deterministic scenario;
- every native strategy, map command, path command, render job, and audio event
  maps to a reviewed source behavior;
- all routes, difficulties, pilots, bosses, endings, failure states, menus,
  save transitions, and long-idle/attract paths have replay fixtures;
- coverage-guided input mutation can no longer discover a new source edge;
- the full replay corpus reports no unexplained semantic, frame, audio, or
  timing divergence;
- the Rust architecture check continues to reject generic memory, processor
  register vocabulary, and source-address execution from shipping crates.

This makes the remaining work measurable. A function ledger entry is complete
only after it advances through the proof ladder; source review or a green unit
test by itself is not enough.
