# Star Fox 2 — Clean-Room Disassembly (Phase 1)

Companion to `docs/SF2_RECON.md`. This phase builds the *disassembly tooling* and
uses it to locate the SF2 logic/geometry dispatchers by decoding the retail ROM's
own machine bytes, using the SF1 opcode grammars (`reference/ultrastarfox/SF/` and
the Rust workspace) as the Rosetta Stone.

**Provenance / clean-room:** everything here is derived from the user-owned retail
ROM (`Star Fox 2 (USA, Europe).sfc`, 1 MB headerless LoROM, file offset == linear
offset) plus the independently-reconstructed SF1 grammars. **No leaked Nintendo
("Gigaleak") source was read or referenced** (per `SF2_RECON.md §5`). Confidence
tags: **certain / high / medium / low**. Where the bytes don't support a mapping,
it is marked unresolved rather than guessed.

Tooling lives in `tools/sf2/disasm/`; run with `nix develop --command python3`.
Generated listings are in the session scratchpad (`sf2d_*` prefix).

---

## 1. What the disassemblers cover

| Tool | Purpose |
|---|---|
| `cpu65816.py` | Table-driven 65816 decoder: all 256 opcodes, all addressing modes, **M/X flag tracking** (SEP/REP) so immediate operand widths (`#$xx` vs `#$xxxx`) decode correctly; resolves branch/JMP/JSR/JSL/BRL targets to CPU long addresses. |
| `disasm_host.py` | Recursive-descent (flow-following) driver: walks from the reset vector and seed roots, follows branches/calls/jumps carrying the (M,X) state along each path, auto-labels call/jump targets, emits annotated listings. Also a linear-window mode for table/region study. |
| `gsu.py` | GSU / Super FX (**GSU-2**) decoder: prefix-modal (ALT1/2/3 `$3D/$3E/$3F`, TO/WITH/FROM register prefixes) with correct per-`(opcode,alt)` mnemonics and immediate lengths (branch +1, IBT/LMS/SMS +1, IWT/LM/SM +2). Mnemonic set + lengths verified against SF1 `reference/ultrastarfox/SF/MARIO/*.MC`. |
| `find_dispatch.py` | Locates indexed-dispatch sites (`JMP/JSR ($tbl,X)`) filtered to the real `TAX`-preceded dispatch idiom; reads each pointed-to table and sizes it. |
| `find_istrat.py` | Scans the whole ROM for stride-4 `[addr16≥$8000][bank][shape]` ISTRAT-record runs. |
| `find_gsu_code.py` | Heuristic GSU-code density scan (prefix/immediate/arith texture vs same-byte-run penalty). |
| `probe_shapes.py` | Validates the SF1 point-block grammar (`04/08 <n> … 0C`) against the 3D banks. |

**Acceptance spot-checks (pass):**
- Reset vector `$FBB8` → file `0x7BB8` decodes to `SEI / CLC / XCE / JML $03E985` —
  the canonical Super-FX-title native-mode entry. Flow-follow yields ~880 sane
  instructions with correct flag-tracked immediate widths.
- The map-VM jump table is located with **83 entries** — a content-scale that
  matches a full level/object script VM (SF1's is comparable), 0000-terminated.

---

## 2. Located routines & tables

### 2.1 Map-VM dispatcher — **HIGH confidence**
The SF1 `newobjex`/`map_exec` analogue. Located at:

| Symbol | CPU | File |
|---|---|---|
| `MAPVM_ENTRY` | `03:8FC9` | `0x18FC9` |
| `MAPVM_DISPATCH` (core) | `03:8FD3` | `0x18FD3` |
| indexed `JMP ($8FE7,X)` | `03:8FE4` | `0x18FE4` |
| **opcode jump table** | `03:8FE7` | `0x18FE7` |

Decoded dispatch core:
```
03:8FD3  SEP #$20
03:8FD5  LDA $192E        ; data-bank byte for the map stream (VM's "map bank")
03:8FD8  PHA / PLB        ; set DBR = map bank
03:8FDA  REP #$20
03:8FDC  LDA $8000,X      ; fetch opcode from map bytecode stream (X = map pointer)
03:8FDF  AND #$00FF       ; isolate opcode byte
03:8FE2  TXY              ; save stream pointer in Y
03:8FE3  TAX
03:8FE4  JMP ($8FE7,X)    ; dispatch (opcodes are EVEN; X indexes words directly)
```
This is byte-for-byte the SF1 map-VM shape: bank-switch to the map data bank held
in a variable, read an opcode from the `$8000,X` stream, dispatch through an
**even-opcode word table**. Handlers begin `TYX` (restore the stream pointer from
Y), read operands via `LDA $8001,X` / `LDA $8000,X`, `INX` to consume them, then
either `JMP $8FD3` to re-dispatch (instant op) or store the pointer and `RTS`
(wait/yield) — the control-flow now implemented by `rust/sf2-map`.

**Table:** 83 word entries → bank-03 handlers clustered in `$98EE–$9FF2`,
opcodes `0,2,4,…,164` (even), `0000`-terminated. Reachability extraction from all
25 retail roots proves **4,094 commands using 22 opcode semantics**, including 232
object spawns and 262 inline routines. Every reachable opcode and every inline
routine now has a typed Rust representation; unknown bytecode is rejected rather
than guessed.

### 2.2 State-machine dispatchers — **medium confidence**
`find_dispatch.py` surfaced several `ASL A / TAX / JMP|JSR ($tbl,X)` dispatchers
that read a **state/mode variable** (not a bytecode stream). These are game-mode
and per-object/animation state machines (relevant to the strat layer):

| Site | Reads | Table | ~entries |
|---|---|---|---|
| `04:A56C` (`JSR ($A571,X)`) | `$D97A` (mode, `==FFFF` guard) | `04:A571` | ~43 |
| `03:C34C` (`JMP ($C34F,X)`) | `$1BE0` | `03:C34F` | ~5 |
| `03:C52E` (`JMP ($C531,X)`) | `$1B78` | `03:C531` | ~10 |
| `03:D9F4` (`JMP ($D9F7,X)`) | `$6C1C,Y` (per-object) | `03:D9F7` | ~7 |
| `04:8FC7` | `$0036,X` (per-object field) | `04:8FCA` | ~14 |
| `04:91DE` (`JSR ($91E3,X)`) | `$0032,X` (per-object field) | `04:91E3` | ~8 |
| `04:C9C3`, `04:CB42` | `$DA69`, `$E089` (modes) | `04:C9C7`,`04:CB47` | ~10, ~7 |

The `$0032,X`/`$0036,X`-indexed per-object dispatchers (`04:91DE`, `04:8FC7`) are
the strongest **ISTRAT/strategy tick** candidates — object state machines keyed off
per-object fields — but confirming the ISTRAT *table* needs §4 tracing.

### 2.3 GSU launch — **high confidence**
GSU execution is started at `02:F84B` (`GSU_LAUNCH`):
```
02:F862  LDA $70390E → STA $3034   ; program bank (PBR) from WRAM var $70:390E
02:F866  STZ $3030
02:F86E  LDA $70390C → TAX → STX $301E   ; entry PC (R15) from $70:390C -> starts GSU
```
GSU-2 is confirmed independently by the `CLSR $3039` clock-select write in boot
(`03:8320`). The entry pointer (`$70:390C`=PC, `$70:390E`=PBR) is **data-driven**:
those vars are only ever *read* at the two launch sites — they are populated
indirectly (indexed store / block copy from a GSU-"job" descriptor table), so the
concrete GSU code offsets are gated on tracing that job table (§4).

---

## 3. SF2 shape / geometry encoding — **exactly extracted**

The earlier bank-entropy inference was wrong. Runtime tracing and pointer
validation locate **577 contiguous 28-byte `ShapeHdr` records** at CPU
`$00:BC9C..$00:FB9B` (end-exclusive `$00:FB9C`). Every referenced point, face,
and BSP stream uses the same Argonaut SHMACS grammar as SF1, including coplanar
face lists and both BSP children. `tools/sf2/extract_shapes.py` validates every
pointer and emits byte-derived Rust data with:

- **11,860 vertices** and **10,524 polygon faces** across the 577 shapes;
- **2 procedural shapes**, explicitly represented rather than mis-decoded;
- point/face streams in ROM banks `$07`, `$0D`, and `$0F`;
- exact landmark shape token `$EA00`: points `$0F:938D`, faces `$0F:93B6`, shift
  4, 18 vertices, and 26 faces.

The table at file `0x8703` is not shape-header data: live GSU polygon-colour
code proves it is the 211-entry, three-byte texture descriptor table. It ends
at the 12-entry coordinate-layout pointer table at `0x897C`; the layouts occupy
`0x8994..0x8A0B`, and the referenced packed-nibble pixels are in source banks
`0x12`, `0x13`, and `0x14`. `rust/sf2-data` now contains all descriptors,
layouts, texture banks, and 577 extracted meshes. `sf-render` selects these
typed flat assets for SF2 faces, with offscreen GPU readback guarding against
the former debug-magenta fallback.

---

## 4. Cross-reference to SF1 (opcode mapping status)

### Map-VM opcode table — **all reachable semantics mapped**
The extractor walks all 25 retail roots and emits 4,094 commands covering the 22
opcodes actually reachable in those scripts. It preserves 232 spawn records, 50
shape tokens, and all 262 inline 65816 routines. The inline blocks are no longer
an arbitrary host callback: they are mechanically classified as 236 calls, 7 word
bit operations, 4 conditional word-bit branches, 8 pilot-linked flag operations,
and 7 GSU-program selections. `rust/sf2-map` executes these typed actions and
validates every continuation against an extraction-proven exit.

### Spawn strategies — **map-facing targets located**
The 232 spawns use four initializer targets: `$06:82ED` (1), `$06:82F9` (1),
`$7F:7E00` (44), and `$7F:7E1E` (186). The bank-`$7F` copied routines map to ROM
`$0A:8000/$0A:801E`; they install per-frame strategy `$7F:7E53` and initialize
the retail object flags and path state. The two bank-06 targets select the player
variant and install `$06:845C`. This supersedes the blind stride-4 ISTRAT scan.

### Path VM — **complete reachable graph extracted**
`$7F:7E53` is the object path/strategy interpreter. Object field `+$2B` points
into CPU bank `$44`; the VM fetches through `[F9]` and dispatches through copied
vectors at `$7F:7EE8` and `$7F:82E8`. Conventional four-byte slots contain a
16-bit `handler - 1` value for the dispatcher's `PHA; RTS`; reachable extended
opcode `$180` proves that the real address calculation also uses a high slot
which aliases the following handler bytes. The 70 map assignments expose 27
unique roots. `extract_path.py` follows every handler CFG, including internal
calls, width changes, the 18-way object-state jump, waits, dynamic pointer
loads, and the shared advance/jump helpers. The closed result is 11,798 commands
from 106 roots, using 274 logical opcode handlers with zero invalid records or
unresolved dispatch targets.

Every one of the 274 reachable dispatch handlers now has a proof-gated semantic
identity and a typed `rust/sf2-path` implementation; no handler-level
`RetailBridge*` operation remains. The 23 former handler bridges are covered by
isolated exact-retail differentials, including pointer-control branches and the
object position/rotation mutations. A semantic name is not counted as lifted
until the reachable path runtime uses the typed operation and the retail
comparison passes.

That does **not** mean the path corpus is fully native yet. Opcode `$089` enters
script-embedded blocks rather than a conventional handler payload. All 42
reachable inline sites now have typed control flow and named operations, and no
generic inline execution escape remains. Twenty simple inline bodies, two dynamic
dispatch blocks, and all 20 named gameplay services are direct Rust with isolated
retail edge differentials. Capture eligibility is also direct Rust across all
cardinal and diagonal boundary modes. Contact classification still delegates its
deep collision refresh to one oracle-only leaf. Thus the mechanically tracked
path staging surface is one named oracle leaf, not zero.
Variable IDs remain retail identifiers in this verification runtime so it can
address SF2's parallel object arrays. None of this address-based staging state is
in the shipping native game's dependency graph.

---

## 5. Current implementation sequence

Completed and mechanically tested:

1. Exact map roots, reachable command graph, opcode semantics, spawns, shapes,
   and typed inline actions in `sf2-data`/`sf2-map`.
2. Exact 577-shape extraction plus shared rendering integration.
3. Exact retail `$7E:B273` draw-record ABI (38-byte records, live count at
   `$7E:18C6`, capacity 64) and renderer bridge.

The next critical path is:

1. Replace the final collision-refresh leaf with typed behavior proven by
   isolated retail comparisons. All 20 named inline gameplay services and the
   capture-eligibility predicate are complete.
2. Decompile the four spawn initializers and their per-frame strategies into
   `sf2-strat`, then validate object/draw state against isolated emulator traces.
3. ~~Extract exact texture descriptors/materials~~ **Complete:** all reachable
   material IDs resolve through exact descriptors/layouts/banks and render in
   the GPU path. Continue broader pixel-level emulator frame comparisons.
4. Recover the strategic-map, player, boss, audio, and progression state machines;
   integrate them behind the game selector and run unattended playthrough oracles.

---

## 6. Artifacts
- `tools/sf2/disasm/extract_map.py` — typed map command and inline-action extractor.
- `tools/sf2/disasm/extract_path.py` — exact reachable path CFG and handler-effect extractor.
- `tools/sf2/extract_shapes.py` — exact 577-shape extractor.
- `rust/sf2-data/src/map.rs` / `path.rs` / `shape_data.rs` / `draw.rs` — generated
  map/path and geometry data plus the retail draw ABI.
- `rust/sf2-map` — strict map VM implementation and runtime tests.
- `rust/sf2-path` — strict path VM with all 274 handlers and all 42 reachable
  inline control blocks typed; only the deep collision refresh remains
  explicitly oracle-staged.
- `tools/sf2/run_mesen_oracle.py` — reproducible disposable-profile Mesen runner;
  avoids Mesen's first-run GUI trap and leaves script artifacts inspectable.
- `tools/sf2/mesen_decompress_oracle.lua` — independent six-stream GSU hash oracle.
- `tools/sf2/mesen_gameplay_probe.lua` — isolated retail draw-list, WRAM, GSU-RAM,
  DMA, geometry, and framebuffer oracle capture.
- `sf2d_boot.asm` — flow disassembly from RESET (+ map-VM & GSU-launch roots).
- `sf2d_symbols.txt` — CPU-addr → label map.
- `sf2d_mapvm.asm` — map-VM dispatcher listing + full 83-entry jump table.
- `sf2d_mapvm_opcodes.txt` — per-opcode handler / operand-size / behavior class.
- `sf2d_gsu_demo.asm` — GSU-2 decoder output (validation; see §2.3 caveat).
