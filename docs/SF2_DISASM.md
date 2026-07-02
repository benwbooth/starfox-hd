# Star Fox 2 — Clean-Room Disassembly (Phase 1)

Companion to `docs/SF2_RECON.md`. This phase builds the *disassembly tooling* and
uses it to locate the SF2 logic/geometry dispatchers by decoding the retail ROM's
own machine bytes, using the SF1 opcode grammars (`reference/ultrastarfox/SF/` and
the ported `src/`) as the Rosetta Stone.

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
(wait/yield) — identical control-flow to `src/game/world.c:map_exec`.

**Table:** 83 word entries → bank-03 handlers clustered in `$98EE–$9FF2`,
opcodes `0,2,4,…,164` (even), `0000`-terminated. Mechanically-derived handler
classification (`sf2d_mapvm_opcodes.txt`): **62 instant "continue" opcodes, 12
"yield/wait" opcodes, 9 unclassified** — the expected map-VM profile. Full table +
listing in `sf2d_mapvm.asm`.

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

## 3. SF2 shape / geometry encoding — **finding: differs from SF1**

The SF1 point-block grammar **does not resolve in SF2.** `probe_shapes.py`
requires *well-formed* blocks (`04/08 <n>` followed by exactly `n·3`/`n·6`
coordinate bytes and a `0C` terminator) and finds essentially **zero** across
banks `0x12–0x17` (0,0,1,0,3,7 total). SF1's open, self-delimiting point/face
format is therefore **not** how SF2 stores geometry. (`SF2_RECON.md`'s "44/6/45…"
figure counted bare `0x04` bytes, not valid blocks.)

Per-2KB entropy across the 3D banks splits them cleanly:
- **Banks `0x12–0x14`: low entropy (2.8–6.4)**, visibly structured. Raw bytes at
  `0x90000` are small nibble-aligned values (`20 20 40 30 30 …`) with recurring
  `0x90`/`0x00` record markers — a **packed/tabular** geometry or transform
  encoding, not SF1 open point-lists.
- **Banks `0x15–0x17`: high entropy (7.3–7.7)** — **compressed or packed**
  (textures / LZ-style shape blobs).

The bank-01 "shape pointer index" at `0x8703` is actually a table of **evenly
0x20-spaced** pointers into bank `0x12` (`12:8000, 12:8020, 12:8040 …`) — i.e.
fixed **32-byte records** (consistent with 4bpp 8×8 graphics tiles or fixed-stride
geometry cells), **not** variable-length SF1 shape headers.

**Conclusion (high confidence):** SF2 uses a **different, denser geometry encoding**
than SF1 — fixed-stride/packed in `0x12–0x14`, compressed in `0x15–0x17`. Pinning
the exact stride and field layout requires disassembling the GSU shape-reader,
which is gated on locating the GSU code stream (§2.3 / §4). The SF1 `shapehdr`
pipeline in `sf-render` will **not** transfer directly; SF2 needs its own decoder.

---

## 4. Cross-reference to SF1 (opcode mapping status)

### Map-VM opcode table — **structure mapped, semantics partial**
`sf2d_mapvm_opcodes.txt` gives, per even opcode `0…164`, its handler address, its
byte-derived operand size (INX-advance count), and its class (continue vs
yield/wait). This is a mechanically-honest table. **SF2's numeric opcodes are
reordered vs SF1** — e.g. SF2 opcode `0` is a *yield/return* (`03:9E6C`:
`TYX / STX $1657 / RTS`), **not** SF1's `MAPOBJ`(spawn, op 0). A full
SF2→SF1-semantic name mapping requires per-handler tracing (identifying the spawn,
wait-by-distance, set-bg, JSR/loop, if/goto handlers by their side effects); that
per-handler decode is the next increment, and is now mechanical given the located
dispatcher + `disasm_host.py`. **No SF2↔SF1 name equivalences are asserted here
that the bytes don't yet support.**

### ISTRAT strategy-pointer table — **unresolved (as `SF2_RECON.md` predicted)**
Blind stride-4 scanning (`find_istrat.py`) yields only false positives in the
sound bank `0x19` (SPC/BRR data coincidentally matching `[addr][bank][shape]`).
The ×4-index-into-long-table sites in banks 02–04 (`03:B799 → 03:B875`, etc.)
resolve to **stage/parameter tables** (values aren't valid `$8000+` code
pointers), not ISTRAT. The ISTRAT table must be reached **from the map spawn
handler**: identify which map opcode allocates an alien and reads its strat index,
then follow that index's table base. That trace is the concrete unblock.

### Path-VM dispatcher — **not located in host banks (low confidence)**
The SF1 path-VM idioms (opcode fetch `LDA $8000,X / AND #$00FF`; ×4 dispatch
`ASL ASL TAX`; `LDA [dp],Y` long-pointer fetch) find **no** match in banks 0–7
beyond the map VM itself. The path VM either lives in a higher bank, is folded into
the per-object state machines of §2.2, or runs as GSU microcode. Unresolved in
phase 1.

---

## 5. Plan for the `sf2-map` / `sf2-strat` crates

**Extractable now (map VM located):**
1. **`sf2-map` opcode skeleton.** Emit the 83-entry table (`sf2d_mapvm_opcodes.txt`)
   as an `Sf2MapOp { opcode, handler_addr, operand_len, kind }` table. The
   interpreter shape (bank-switch → `$8000,X` fetch → even-opcode dispatch →
   operand read → advance/redispatch-or-yield) is confirmed identical to
   `src/game/world.c:map_exec`, so the shared VM *interpreter* transfers; only the
   opcode→handler numbering is SF2-specific.
2. **Handler-semantics pass (mechanical).** For each of the 83 handlers, flow-decode
   with `disasm_host.py` and record its side effects (which `$1xxx` vars it writes,
   whether it calls the object allocator, its operand layout). This produces the
   SF2→SF1 semantic mapping and, as a by-product, **locates the ISTRAT table** (via
   the spawn handler) and the level/`mapdef` tables (via the JSR/goto handlers).
3. **Level-script extraction.** Once the spawn/end/wait opcodes are named, walk each
   level's bytecode from its entry (the map-stream pointers loaded into `X` +
   `$192E`) to dump per-level scripts, mirroring the SF1 `levels.c` port.

**`sf2-strat`:** the per-object state-machine dispatchers (`04:91DE` on `$0032,X`,
`04:8FC7` on `$0036,X`) are the strategy-tick entry points; enumerating their
tables + the ISTRAT table (from step 2) gives the strategy roster. Depends on
step 2.

**Still needs deeper tracing (not blocked, but sequential):**
- **ISTRAT table** → from the map spawn handler (step 2).
- **Path-VM** → decode the object state machines / locate the fetch site (§4).
- **GSU code + shape decoder** → trace the `$70:390C/$390E` GSU job table to get
  code offsets, then disassemble the shape-reader with `gsu.py` to pin the
  `0x12–0x14` fixed-stride and the `0x15–0x17` compression. This is the gate for
  `sf2-render` geometry (the SF1 shape pipeline does **not** transfer — §3).

---

## 6. Artifacts (session scratchpad, `sf2d_` prefix)
- `sf2d_boot.asm` — flow disassembly from RESET (+ map-VM & GSU-launch roots).
- `sf2d_symbols.txt` — CPU-addr → label map.
- `sf2d_mapvm.asm` — map-VM dispatcher listing + full 83-entry jump table.
- `sf2d_mapvm_opcodes.txt` — per-opcode handler / operand-size / behavior class.
- `sf2d_gsu_demo.asm` — GSU-2 decoder output (validation; see §2.3 caveat).
