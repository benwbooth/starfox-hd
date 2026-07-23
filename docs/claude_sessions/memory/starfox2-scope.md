---
name: starfox2-scope
description: "Star Fox 2 port scope — ROM in repo, recon done, blocked on acquiring a disassembly"
metadata: 
  node_type: memory
  type: project
  originSessionId: 8f7b8292-fbc0-48f8-8506-8b6a0949123b
---

Star Fox 2 is a stated user goal (port both SF1 + SF2). ROM "Star Fox 2 (USA, Europe).sfc" (1MB final-release build) is in the repo root. Recon complete 2026-07-02: see `docs/SF2_RECON.md`.

**Key findings:** SF2 = LoROM + Super FX 2, shares the Argonaut 3D core with SF1. SPC audio format is **byte-identical to SF1** (sf-audio reuses directly). Color tables (bank 01 @0x8000), 3D shape point/face data (banks 0x12-0x17), and text tables located. Map-VM opcodes and ISTRAT strategy tables are reordered/extended vs SF1 and do NOT resolve by byte-scanning.

**Blocker / how to apply:** SF2 port phases 1-3 (data extraction, audio, shapes) are unblocked today since formats match SF1. Phases 4-7 (VM opcode mapping, levels, engine deltas: strategic map layer, all-range arenas, walker transform, dogfight AI, 2-pilot Miyu/Fay select, base defense) need a source/disassembly reference — the **2020 Nintendo "Gigaleak" SF2 source tree** is the highest-value target (retail ROM is symbol-stripped). Ask Ben to acquire that to enable the same transcription workflow ultrastarfox enabled for SF1.

**Architecture:** SF2 slots into the Rust workspace as data crates (sf2-map/strat/path/data) + an engine-delta crate behind a `Game` selector in sf-core, sharing sf-render/sf-audio/sf-difftest. Sequenced AFTER the SF1 RIIR finish. [[riir-decision]] [[overhaul-phase2-status]]
