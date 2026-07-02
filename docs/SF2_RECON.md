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
| 01 | 008000–00FFFF | 7.1 | **Color/material tables** `0x8000–0x86F1` + **shape/graphics pointer index** `0x8703–0x8960` — high |
| 02–04 | 010000–027FFF | 6.7 | 65816 code + data; **enemy/boss name tables** in bank 03 (`0x187D6`, `0x18941`) — high |
| 05–07 | 028000–03FFFF | 6.2–6.8 | mixed code/data — medium |
| 08–0A | 040000–057FFF | 6.6–7.2 | 2D graphics: font / HUD / portrait / strategic-map tiles (high printable density) — medium |
| 0B–11 | 058000–08FFFF | 5.8–7.0 | code + tables + graphics — low/medium |
| 12–17 | 090000–0BFFFF | 6.4–7.6 | **3D shape data** (point + face lists; 200+ point-blocks concentrated here) — high |
| 18 | 0C0000–0C7FFF | 7.4 | graphics/data — low |
| 19–1F | 0C8000–0FFFFF | 7.0–7.6 | **SPC sound driver + BGM/SE sequences + BRR samples**; driver blob at `0xCBE1E` — high |

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
| SPC driver upload blob | `0xCBE1E` | `[len][dest]` chain: 6 blocks incl **dest `$0400`** (driver code) + `$2593/$1D03` (samples) + `$4E0A/$2DE8` (DSP/echo) + `$EC00`; terminator `00 00 00 04` @ `0xD2E7D` | high |
| SPC song / sample / SE blobs | `0xCBE1E`–`0xFFDD1` | ~30 `[len16][dest16]…00 00 00 04` chains; dests cluster at `$3064/$9800` (BGM control+seq), `$3Exx` (DSP), `$Exxx` (ARAM seq), BRR sample banks | high |
| 65816 APU uploader (`LDA #$BBAA` IPL handshake) | `0xA710D`, `0xEF466` | Opcode `A9 AA BB` | high |
| APU start byte `LDA #$CC` | `0x37967`, `0xA4D07`, `0xA7508` (+others) | Opcode `A9 CC` | medium |
| Color / material tables | `0x008000`–`0x0086F1` (bank 01) | Word runs with high byte `$3E` (coldepth) / `$3F` (colnorm); a 196-word master table at `0x806C` | high |
| Shape / graphics pointer index | `0x008703`–`0x008960` (bank 01) | Six stride-3 pointer arrays into banks `$12/$13/$14` | medium |
| 3D shape point/face data | banks **`0x12`–`0x17`** (`0x90000`–`0xBFFFF`) | Point-block grammar `04 <count> <count×3 signed> 0C`: 44/6/45/25/36/38 hits in banks 12/13/14/15/16/17 | high |
| Text: title / HUD (`NINTENDO PRESENTS`, `YOU LOST`, `CONTINUED`, `CORNERIA`) | `0x003083` | ASCII, null-terminated | high |
| Text: staff-roll / credits | `0x0038BA` | ASCII w/ control-byte prefixes; scroll record table at `0x38A0` | high |
| Text: boss/enemy display names | `0x0187D6` | space-padded, null-terminated list (`MOTH GLIDER`,`HAL BIRD`,`ANDORF`,`ANDROSS`,…) | high |
| Text: character/rival names | `0x018941` | tight null-terminated list (`ALGY`,`PIGMA`,`LEON`,`WOLF`,…) | high |
| Text: mission/briefing strings | `~0x001E00`–`0x003100` | ASCII (`MISSION`,`TRAINING`,dialogue fragments) | high |

### Text/message VM format (decoded)
Strings are **uppercase ASCII, null (`0x00`) terminated**, each preceded by a **control
byte** (observed `0x22 " `, `0x23 #`, `0x26 &`, `0x2D -`, `0x2E .`, `0x2F /`, plus `0x04`
as a line/pause command). These select centering / color / row for the staff-roll and menu
text — the SF2 analogue of SF1's `MSG/` message tables. Note SF2 retains **both** `ANDORF`
and `ANDROSS` name strings adjacently (regional-naming carryover).

### SPC upload-block format (shared with SF1, confirmed byte-for-byte)
Each blob = chain of `[len:2 LE][dest:2 LE][data…]` blocks, terminated by `00 00` (len=0)
followed by the entry word `00 04` (**$0400**, the driver entry). SF1's uploader (`ASM/SOUND.ASM`)
uses the `$BBAA` IPL ready handshake then `$CC` start byte; SF2's uploader at `0xA710D`
matches. This means **the SF1 audio pipeline in `sf-audio` is directly reusable for SF2** —
only the blob offsets and the sequence/instrument payloads differ.

### What is NOT precisely located (low confidence)
- **Map/level bytecode VM tables** and **ISTRAT strategy pointer tables.** SF1's signatures
  (even opcodes `$00–$8C`, `$8A xx` short-waits, stride-4 `[addr16][bank][shapeidx]` ISTRAT
  records, stride-3 `mapdef` level tables) do **not** cleanly resolve in SF2 by blind scan:
  SF2 reorders and extends both VMs, and `$8A`/even-byte density is diffuse across code and
  graphics banks. Locating these needs a live disassembly / GSU+65816 trace, not a byte scan.
  The SF1 **encoding grammars** (documented in §5) remain the ground-truth template once a
  disassembly pins the dispatch tables. Expect the map/strat data to live in banks `0x02–0x07`
  alongside the host code, referenced by 3-byte pointers.

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
| **Walker transformation** (Arwing ↔ ground walker) | `sf-strat` player + `sf-render` | Two player shapes with a morph animation (the `WALKER_L`/`WALKER_R` shape family exists already in SF1 `SHAPES/`); add a transform state + control mode toggle. |
| **Dogfight / boss AI** (homing, evasion, formation) | `sf-strat` enemy | New strategy routines; grammar is the same ISTRAT/path VM. |
| **2-pilot select** (choose 2 of Fox/Falco/Peppy/Slippy/**Miyu/Fay**) + per-pilot stats & levelling | `sf-game` | New save/roster state + stat modifiers on player strat; menu screens (text VM + portrait tiles in graphics banks). |
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

**Known community references to acquire** (names only — not fetched here). Any of these
would enable the same ASM→C→Rust transcription workflow:
- **The Star Fox 2 prototype-source leak / "Nintendo Gigaleak" (2020)** — reportedly included
  SF2 build trees with Super FX (GSU) source and assets; the highest-value target because it
  is the actual sources (the SF1 analogue of what `ultrastarfox` reconstructs).
- **`ultrastarfox` upstream project** — the SF1 reconstruction already in `reference/`;
  its authors and the wider **StarGraphics / Star Fox disassembly community** are the natural
  home for an SF2 counterpart.
- **Community SF2 disassemblies / decompilation efforts** hosted on GitHub and the
  SNESdev / romhacking communities (search "Star Fox 2 disassembly", "Super FX GSU
  disassembler").
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
4. The build's data-file manifest (which bank holds which shape/level/song), to replace the
   entropy-derived bank map above with exact boundaries.

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

**Phases:**
1. **Extraction harness.** Reuse this doc's block scanners as a `sf-tools` extractor: dump
   the SPC blobs (`0xCBE1E+`), color tables (`0x8000`), 3D shape point/face data (banks
   `0x12–0x17`), and text tables. Verify blob round-trips (re-pack → byte-identical). Cheapest
   win because the formats are already SF1-compatible. **Blocks on: nothing.**
2. **Audio bring-up.** Feed extracted SF2 SPC blobs through the existing `sf-audio` uploader
   (protocol identical). Validate against an emulator SPC dump. **Blocks on: phase 1.**
3. **Shape/render bring-up.** Load SF2 shapes into `sf-render` via the shared `shapehdr`
   pipeline; the color-word decode already exists. Diff rendered models vs emulator frames in
   `sf-difftest`. **Blocks on: phase 1; needs the shape header/index format pinned — medium
   risk until a disassembly confirms bank-01 header layout.**
4. **VM opcode mapping.** Using an acquired SF2 disassembly (§5), pin the map/strat/path
   dispatch tables and transcribe SF2's opcode→handler numbering onto the shared VM crates as
   `sf2-*` tables. This is the **critical-path blocker** — without a reference this stays
   reverse-engineering-by-trace. **Blocks on: acquiring a source/disassembly reference.**
5. **On-rails levels.** With VMs mapped, port SF2's rail levels (reuse SF1 rail-camera logic
   in `sf-map`/`sf-game`). Difftest against emulator. **Blocks on: phase 4.**
6. **Engine deltas.** Implement all-range camera, walker transform, dogfight AI, 2-pilot
   select in `sf2-strat`/`sf2-meta`. **Blocks on: phase 5.**
7. **Strategic layer.** Build `sf2-meta` (Corneria defense, mission graph, base HP, roster
   levelling) on top. Net-new; validate against gameplay. **Blocks on: phase 6.**

**Sequencing note:** phases 1–3 (data extraction, audio, shapes) are unblocked *today*
because they ride SF1-identical formats. Phases 4–7 are gated on acquiring an SF2
source/disassembly reference (§5) to resolve the reordered VM opcodes and the strategic-map
logic — the one thing byte-scanning cannot recover.
