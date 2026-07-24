# Star Fox 2 (USA, Europe) ROM Reconnaissance

Target: `Star Fox 2 (USA, Europe).sfc` (repo root), the finalized 2017 SNES Classic
release build. This document maps SF2's ROM structure against the Star Fox 1 engine we
already understand from `reference/ultrastarfox/SF/` and lays out a porting plan into the
existing `rust/` workspace.

All offsets in this doc are **file offsets** (the ROM is headerless / 1,048,576 bytes, so
file offset == LoROM linear offset). LoROM bank _b_ occupies file `b*0x8000 .. b*0x8000+0x7FFF`
and is CPU-addressable at `$b0:8000` (and mirrors). Confidence tags: **certain / high /
medium / low**.

---

## 1. ROM basics

| Field | Value | Notes |
|---|---|---|
| File size | 1,048,576 B (1 MB, headerless) | 32 × 32 KB LoROM banks |
| Internal title (`0x7FC0`) | `STARFOX2` | |
| Map mode (`0x7FD5`) | `0x20` | **LoROM** |
| Cartridge/chipset (`0x7FD6`) | `0x15` | ROM + **Super FX** + RAM + battery |
| ROM size (`0x7FD7`) | `0x0A` | 1 Mbit-code = 1 MB |
| RAM size (`0x7FD8`) | `0x00` | GSU work-RAM reported via mapper, not here |
| Country (`0x7FD9`) | `0x01` | USA/NTSC |
| Dev ID (`0x7FDA`) | `0x33` | → use extended header |
| Version (`0x7FDB`) | `0x00` | |
| Checksum / complement | `0x8E27` / `0x71D8` | **valid** (computed sum16 = `0x8E27`) |
| Ext. header maker (`0x7FB0`) | `"01"` | Nintendo |
| Ext. header game code (`0x7FB2`) | `"XJ  "` | SF2 |

**Silicon:** this is a **Super FX 2 (GSU-2)** title — same mapper family as SF1 but the
larger 21 MHz revision. SF2's rendering, matrix math, and 3D object interpreter run as GSU
microcode; the 65816 is the shell/host (I/O, DMA, audio kickoff, game-state glue), exactly
as in SF1 (`reference/ultrastarfox/SF/MARIO/*.MC` is the SF1 GSU source).

**Vectors (bank 0):**
- Emu `RESET` = `$FBB8` → 65816 boot code lives at file `0x7BB8` (bank 0).
- Native `NMI` = `$0108`, native `IRQ` = `$010C` — **RAM stubs**: SF2 copies its NMI/IRQ
  handlers into low WRAM at boot (standard for Super FX titles so the CPU keeps servicing
  interrupts while GSU owns the ROM bus). COP/BRK/ABORT = `$FBBF`.

**65816 vs GSU vs data split:** bank 0 is 65816 host code (reset + the Argonaut 3D engine
shell). The engine self-identifies: the ASCII tag **`STAR GLIDER 01 NOV 1991`** sits at
file `0x000E07` (Argonaut's StarGlider-derived 3D core, dated). Per-bank entropy (below)
separates code-ish banks (~5.8–6.8 bits/byte) from packed graphics/BRR-sample banks
(~7.1–7.6).

---

## 2. Bank map

Per-bank Shannon entropy + identified contents. "GSU-visible" = data the Super FX reads
directly (shapes, matrices, tables).

| Bank | File range | Entropy | Identified contents (confidence) |
|---|---|---|---|
| 00 | 000000–007FFF | 6.8 | 65816 host code; RESET `0x7BB8`; engine tag `0xE07`; text/message data (title/HUD `0x3083`, credits `0x38BA`, mission text `~0x1E00`) — high |
| 01 | 008000–00FFFF | 7.1 | **Color/material tables** `0x8000–0x86F1`; **577 contiguous 28-byte ShapeHdr records** at CPU `$00:BC9C..$00:FB9B` — certain |
| 02–04 | 010000–027FFF | 6.7 | 65816 code + data; **enemy/boss name tables** in bank 03 (`0x187D6`, `0x18941`) — high |
| 05–07 | 028000–03FFFF | 6.2–6.8 | mixed code/data — medium |
| 08–0A | 040000–057FFF | 6.6–7.2 | 2D graphics: font / HUD / portrait / strategic-map tiles (high printable density) — medium |
| 0B–11 | 058000–08FFFF | 5.8–7.0 | code + tables + graphics — low/medium |
| 12–14 | 090000–0A7FFF | 6.4–7.6 | **Exact packed-nibble polygon-texture banks**, selected by the descriptor table at file `0x8703` — certain |
| 15–17 | 0A8000–0BFFFF | 6.4–7.6 | packed graphics/tile and other asset data; an earlier shape-bank classification was disproved by pointer tracing — medium |
| 18 | 0C0000–0C7FFF | 7.4 | graphics/data — low |
| 19–1F | 0C8000–0FFFFF | 7.0–7.6 | **SPC sound driver + BGM/SE sequences + BRR samples**; exact reset-path driver upload starts at `0xD0000` — certain |

(Boundaries are approximate; several banks interleave code, tables, and packed graphics.)

---

## 3. Located structures (offset table)

Signatures were derived from the SF1 reconstruction and pattern-matched against SF2's bytes.

| Structure | File offset | How located | Confidence |
|---|---|---|---|
| Cartridge header | `0x7FC0` | Standard SNES header, valid checksum | certain |
| Ext. header | `0x7FB0` | Maker/game-code ASCII | certain |
| 65816 reset code | `0x7BB8` | Emu reset vector | certain |
| Engine tag `STAR GLIDER 01 NOV 1991` | `0x000E07` | ASCII scan | certain |
| SPC driver upload file | `0xD0000`–`0xD2E81` | Reset calls the uploader with CPU `$1A:8000`; exact subchain has 7 blocks to `$3100/$31BA/$2FE8/$0400/$4E0A/$2DE8/$EC00`, then entry `$0400` | certain |
| SPC upload catalog | `0xCBE1E`–`0xFFEC3` | 62 broad `[len16][dest16]…00 00 00 04` regions; later host tables resolve exact upload starts within this catalog | certain |
| Host audio-program table | `0x1E495`–`0x1E724` | 50 variable-length records; each supplies a preload command, port-3 cue, and exact upload pointers | certain |
| 65816 APU uploader (`LDA #$BBAA` IPL handshake) | `0xA710D`, `0xEF466` | Opcode `A9 AA BB` | high |
| APU start byte `LDA #$CC` | `0x37967`, `0xA4D07`, `0xA7508` (+others) | Opcode `A9 CC` | medium |
| Color / material tables | `0x008000`–`0x0086F1` (bank 01) | Word runs with high byte `$3E` (coldepth) / `$3F` (colnorm); a 196-word master table at `0x806C` | high |
| Polygon texture descriptors/layouts | `0x008703`–`0x008A0B` | Live GSU polygon-colour routine indexes 211 three-byte descriptors at `$8703` and 12 layout records through pointer table `$897C`; all 52 IDs referenced by 126 decoded shapes resolve | certain |
| 3D shape headers | CPU `$00:BC9C..$00:FB9B` | 577 contiguous, pointer-validated 28-byte `ShapeHdr` records | certain |
| 3D shape point/face data | CPU banks `$07/$0D/$0F` | Full BSP traversal validates the SF1 SHMACS point/face grammar; 11,860 vertices, 10,524 faces, 2 procedural shapes | certain |
| Retail draw list | WRAM `$7E:B273`, 38-byte records; count `$7E:18C6` | Write tracing of copied builder `$7F:9201..$7F:947D` (ROM `$02:9201..$02:947D`) | certain |
| Map bytecode | 25 roots, 4,094 reachable commands | Dispatcher `$03:8FD3`, 22 reachable semantics, 232 spawns, 262 typed inline routines | certain |
| Text: title / HUD (`NINTENDO PRESENTS`, `YOU LOST`, `CONTINUED`, `CORNERIA`) | `0x003083` | ASCII, null-terminated | high |
| Text: staff-roll / credits | `0x0038BA` | ASCII w/ control-byte prefixes; scroll record table at `0x38A0` | high |
| Text: boss/enemy display names | `0x0187D6` | space-padded, null-terminated list (`MOTH GLIDER`,`HAL BIRD`,`ANDORF`,`ANDROSS`,…) | high |
| Text: character/rival names | `0x018941` | tight null-terminated list (`ALGY`,`PIGMA`,`LEON`,`WOLF`,…) | high |
| Text: mission/briefing strings | `~0x001E00`–`0x003100` | ASCII (`MISSION`,`TRAINING`,dialogue fragments) | high |
| Difficulty selection | CPU `$03:C397..$03:C3CE` | Normal/Hard use ordinals 0/1; Expert ordinal 2 is exposed only when cartridge progress flag `0x10` is set | certain |
| Expert unlock | CPU `$0D:F777..$0D:F792`, helper `$0B:F115` | A zero-damage Hard clear sets and persists progress flag `0x10`; Normal and damaged Hard clears do not | certain |
| Difficulty campaign profiles | CPU `$04:DFD1..$04:E014`, activation `$04:CE78`, wave data `$04:E6F2` | Normal/Hard/Expert begin with 2/3/3 occupied planets, 2/3/6 planetary-defense units, 2/4/4 opening attackers, and 1/1/2 Battle Carriers. Normal has one two-attacker opening wave; Hard and Expert have two | certain |
| Difficulty strategic schedules | CPU `$04:EF74`, starts `$04:E04C` | Distinct Normal/Hard/Expert event streams contain 6/13/15 timed events before their terminators; verified by `tools/sf2/verify_difficulty_profiles.py` | certain |
| Campaign world assignments | CPU `$04:EE52`, retail command-map labels | Retail labels selections 0–5 as Venom, Titania, Macbeth, Eladard, Meteor, and Fortuna. Normal selects all six pairs from the Venom/Titania/Eladard/Meteor pool; Hard and Expert select all 20 three-world combinations. Expert rows retain a full six-world permutation after the occupied prefix | certain |
| Campaign world mission/audio entries | Independent saved-map replays, `tools/sf2/fixtures/campaign_world_entries.trace` | Venom, Titania, Macbeth, Eladard, Meteor, and Fortuna select distinct retail audio records `$062/$076/$08A/$09E/$0B2/$0C3`; the four newly reached worlds also enter distinct setup maps, active maps, and player spawns | certain |
| Meteor Wall Spider encounter | Three hash-bound saved-state replays, `tools/sf2/fixtures/wall_spider.trace` | From an isolated saved state, natural right/forward movement arms the dormant core; an accelerated exact-actor hit advances its retail damage/death paths, decrements both occupied-world mirrors from 2 to 1, and leaves Select's four-form Walker-to-Arwing transition intact. The saved-state ancestry used forced base flags, controller durability, and teleports, so this certifies the encounter mechanics but not its unforced campaign route | certain for encounter mechanics; route unknown |
| Meteor Queen Dragoon encounter | Five hash-bound saved-state and callback replays, `tools/sf2/fixtures/meteor_queen_dragoon.trace` | One body and four linked components, durability, destruction shapes, and actor movement callbacks are verified. Mission-state-unforced replays naturally defeat Queen Dragoon, follow its dropped switch, reduce the global occupied-world mirrors from 2 to 1, open and traverse the installation, complete the sortie, and return to the strategic map with one occupied campaign world remaining. The mirrors are campaign-global, not two Meteor-local objectives, so this evidence does not prove that Queen Dragoon and Wall Spider are separate sorties. The older forced-zero-counter replay proves only the resulting return presentation | certain |

### Text/message VM format (decoded)
Strings are **uppercase ASCII, null (`0x00`) terminated**, each preceded by a **control
byte** (observed `0x22 " `, `0x23 #`, `0x26 &`, `0x2D -`, `0x2E .`, `0x2F /`, plus `0x04`
as a line/pause command). These select centering / color / row for the staff-roll and menu
text — the SF2 analogue of SF1's `MSG/` message tables. Note SF2 retains **both** `ANDORF`
and `ANDROSS` name strings adjacently (regional-naming carryover).

### SPC upload-block format (shared with SF1, confirmed byte-for-byte)
Each exact host-selected upload file is a chain of `[len:2 LE][dest:2 LE][data…]`
blocks, terminated by `00 00` (len=0) followed by an entry word. SF1's uploader
(`ASM/SOUND.ASM`) uses the `$BBAA` IPL ready handshake then `$CC` start byte;
SF2's reset uploader at CPU `$03:E409` uses the same protocol and receives CPU
pointer `$1A:8000` (file `0xD0000`) for the driver. The earlier `0xCBE1E`
classification was too broad: it is the beginning of a larger catalog region,
not the driver file passed by reset. The offline `sf-audio` oracle reuses the
upload protocol, while the shipping runtime plays semantic PCM rendered from it.

### Remaining precisely-located but not yet fully implemented logic

- The map VM is extracted and implemented for every command reachable from all 25 retail
  roots. Its 232 spawns resolve to four initializer targets: `$06:82ED`, `$06:82F9`,
  `$7F:7E00`, and `$7F:7E1E`.
- The general object path interpreter at copied WRAM `$7F:7E53` is mechanically extracted:
  106 roots produce a closed graph of 11,798 commands and 274 logical opcode handlers.
  Handler CFG analysis resolves every static advance, branch, wait, return, and
  dynamic-pointer exit with no invalid records. All handlers have proof-gated identities;
  23 remain explicitly isolated behind the oracle-only retail bridge.
- Strategic-map, player, mission, boss, and progression state machines now have native
  typed implementations, but their remaining PARTIAL audit rows still require the same
  trace-and-decompile pass before full parity can be claimed. The player has retail pilot
  profiles, distinct rapid/charge weapon lifecycles, exact active/transform/Walker forms,
  and a shared Select-driven transformation state; Walker turn easing and jump wind-up
  remain explicitly open. Polygon texture descriptors, layouts, and all three source banks
  are exact generated data consumed by the native renderer.

---

## 4. Engine deltas — SF2 additions vs the shared SF1 core

**Shared with SF1 (reuse the existing architecture):**
- Super FX 3D object interpreter: same `shapehdr` header (points/faces/scale/colbox/color-
  ptr/LODs), same point-block (`04/08 … 0C`) and face-record (`[N][col][vis][nx][ny][nz][idx…]`,
  `FE`/`FF` terminators) grammar, same color-word discriminators (`≤0x3D` light, `0x3E`
  depth, `0x3F` flat, `0x8000` anim, `0x4000` texture, `0xC000` smooth). → `sf-render`
  shape pipeline transfers.
- Object / strategy / path VM *grammars* (`sf-strat`, `sf-path`): the ISTRAT record shape,
  the path 1-byte-opcode-into-×4-dispatch-table, and the `mapobj`/`mapwait`/spawn encodings
  are the same family (opcode numbers differ).
- SPC700 driver + `[len][dest]` upload (`sf-audio`) — identical protocol.
- Map/level bytecode VM (`sf-map`) — same grammar family.

**New in SF2 (needs engine-delta modules):**

| SF2 feature | Maps onto | Delta work |
|---|---|---|
| **Strategic map layer** (real-time Corneria-defense overworld: missiles inbound, base HP, node travel, mission select) | new `game-mode` above the existing per-level map VM | New meta-state machine + timer/economy model; the 2D strategic-map graphics are the tile banks (`0x08–0x0A`). No SF1 equivalent — net-new module. |
| **All-range (free-flight) arenas** | `sf-map` + `sf-strat` | Camera/movement changes from on-rails to 6-DoF arena; enemy `sf-strat` behaviors gain pursue/orbit instead of scripted spawn waypoints. |
| **Walker transformation** (Arwing ↔ ground walker) | `sf2-game` typed player state + `sf-render` | Implemented as a shared flat `PlayerCraftForm`: Select drives exact class-specific two-stage meshes and retail-sampled timing in ground missions. Walker movement/fire are connected; exact turn easing and jump wind-up remain open. |
| **Dogfight / boss AI** (homing, evasion, formation) | `sf-strat` enemy | New strategy routines; grammar is the same ISTRAT/path VM. |
| **2-pilot select** (choose 2 of Fox/Falco/Peppy/Slippy/**Miyu/Fay**) + per-pilot stats & levelling | `sf-game` | Typed roster state + stat modifiers on player strat; menu screens (text VM + portrait tiles in graphics banks). Typed campaign progress now preserves the retail zero-damage Hard-clear Expert unlock in `starfox2.save`. |
| **Base-defense / timed objectives** | `sf-map` + strategic layer | Objective/score conditions; ties into `mapif`/`mapjmpvar`-style VM ops. |
| **Branching mission structure** (difficulty-scaled routes) | level table / `sf-map` | Larger, data-driven course graph vs SF1's fixed 3 paths. |

---

## 5. Source-availability & recommended reference acquisition

**Symbol strings / debug remnants in the ROM:** the finalized retail build is **stripped** —
no symbol table, source paths, or debug label strings survive. What *is* present: the full
**staff-roll credits** (`0x38BA`: Dylan Cuthbert, Argonaut Software Ltd, Pete Warnes, Carl
Graham, Giles Goddard-era team, etc.), the `STAR GLIDER 01 NOV 1991` engine tag, and all
UI/enemy **name text**. These confirm shared authorship/lineage with SF1 but give no
code-level symbols. Transcription therefore needs an external reference, exactly as
`ultrastarfox` provided for SF1.

**Reference-acquisition options** (names only — not fetched here). This project is a
**clean-room reverse-engineering effort** (ROMs the user owns + independently reconstructed
ASM). To preserve that legal footing for the whole codebase, the SF2 reference MUST be of
legitimate provenance — reverse-engineered from the retail ROM, NOT the leaked Nintendo
source. Do NOT use the 2020 "Gigaleak" SF2 source tree: it is stolen proprietary Nintendo
code, and incorporating it would contaminate the clean-room status of the entire port
(SF1 included), on top of the copyright issue. Legitimate options:
- **The ROM itself is the ground truth.** A GSU-2 + 65816 disassembly produced *from the
  retail ROM*, using the SF1 opcode grammars in §5 as a Rosetta Stone, is the honest route —
  exactly how the SF1 coverage was built. Slower than transcribing a ready-made source tree,
  but clean.
- **`ultrastarfox` upstream project** — the SF1 reconstruction already in `reference/`;
  its authors and the wider **Star Fox disassembly community** are the natural home for an
  independently reverse-engineered SF2 counterpart.
- **Community SF2 disassemblies** — usable ONLY if independently RE'd from the ROM (verify
  provenance before pulling anything in; reject any that derive from the Gigaleak).
- **General Super FX / GSU tooling**: a GSU-aware disassembler and the published GSU opcode
  documentation (SNESdev wiki) — required because ~half of SF2's logic is GSU microcode that
  a 65816-only disassembler cannot read.

**What such a reference must contain to be useful here:**
1. GSU microcode source or a GSU disassembly (the 3D interpreter, matrix/transform, and
   object-render loops — SF2's `MOBJ.MC` equivalent).
2. 65816 host-side source: the map-VM dispatcher and opcode table, the ISTRAT table, the
   path-VM dispatch table, the strategic-map state machine, and the audio uploader — so the
   **numeric opcode values** (which differ from SF1) can be pinned to the grammars in §5.
3. Symbol names for shapes, strategies, paths, levels, and messages so extracted data can be
   labelled rather than numbered.
4. Exact strategic-map, texture, path, and audio symbol names would improve readability,
   but implementation continues from retail-ROM traces without requiring such a reference.

---

## 6. Phased porting plan (into the existing `rust/` workspace)

The workspace already has `sf-core, sf-path, sf-map, sf-strat, sf-game, sf-render, sf-audio,
sf-app, sf-difftest`. **The renderer, audio, shell, and difftest harness are game-agnostic
and should be shared.** Add SF2 as data crates + engine-delta modules behind a game-select
enum, not a fork.

**Proposed structure:**
- Introduce a `Game { StarFox1, StarFox2 }` selector in `sf-core`; thread it through
  `sf-app`. `sf-render`, `sf-audio`, `sf-difftest` stay shared (formats are identical).
- New data crates mirroring the SF1 pattern: `sf2-map`, `sf2-strat`, `sf2-path`, `sf2-data`
  (shapes/colors/text), each depending on the shared VM crates for the *interpreters* and
  supplying SF2's *opcode tables + data blobs*.
- New engine-delta crate `sf2-meta` for the strategic-map / mission / roster layer (no SF1
  analogue).

**Current phases:**
1. **Complete:** exact shape extraction (577 shapes), exact color-table words, retail draw
   ABI, map roots/commands/spawns, typed inline map actions, and the closed 27-root object
   path graph (11,798 commands / 274 opcode handlers).
2. **In progress:** 251 of 274 reachable path handlers execute as typed staging operations;
   replace the 23 explicit oracle-backed bridge operations and decompile the four spawn
   initializers and per-frame strategies into native typed gameplay systems.
3. **Complete:** exact 211-entry texture descriptor table, all 12 coordinate layouts,
   three packed-nibble banks, reachability validation, and offscreen GPU readback.
   Continue pixel-level emulator frame diffs and audio-oracle comparison.
4. **In progress:** strategic-map, all-range player/Walker, boss/dogfight, roster,
   objectives, and progression state machines are native and behind the shared game
   selector. Continue oracle-backed behavioral closure; the Walker transformation itself is
   descriptor- and frame-verified, while its ground movement dynamics remain partial.
5. **Acceptance:** unattended retail-oracle scenarios plus complete workspace tests and both
   SF1/SF2 playable-route regression. No phase is gated on leaked or external source.
