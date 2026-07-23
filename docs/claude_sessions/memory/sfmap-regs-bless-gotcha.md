---
name: sfmap-regs-bless-gotcha
description: Re-wiring a sf-map boss/obj placement that changes bytecode LENGTH needs a manual .regs.txt update — SF_BLESS_FIXTURES only rewrites the .bin
metadata: 
  node_type: memory
  type: project
  originSessionId: 8f7b8292-fbc0-48f8-8506-8b6a0949123b
---

When you change a `sf-map` level placement (e.g. rewire a boss `mapobj` from a
small istrat index to a 24-bit synthetic strat address like `STRAT_ADDR_BOSSH`),
the byte encoding grows: `mapobj` uses the COMPACT form only when both shape≤0xFF
AND strat≤0xFF; a strat address > 0xFF falls back to `mapnobj` (NORMOBJ) =
shape 2 bytes + strat 3 bytes, so a `IS_BOSS2`(1-byte)→`0x0600xx`(3-byte) swap
grows the map by +3 bytes (builder.rs `mapobj`/`mapnobj`).

`SF_BLESS_FIXTURES=1` for the `route1_parity`/`fixture_parity` tests **only
rewrites `<name>.bin`** — the comment "lengths unchanged, so .regs.txt stays
valid" is FALSE once the length changes. You must ALSO hand-edit
`tests/fixtures/<name>.regs.txt`:
- `length N` → the new .bin size
- every `inline <offset>` (and `native`) that sits AFTER the insertion point
  shifts by the byte delta.

Get the exact values from the builder instead of guessing: a throwaway test that
calls `route1::get_full(map_id::M1_x)` and prints `l.level.data.len()` +
`l.inline_regs.iter().map(|&(p,_)|p)` + `l.native_regs`. The test asserts
length==bin.len(), data==bin, AND native/inline ORDER.

Also: `--test "*"` does NOT glob cargo test targets — it runs nothing/errors.
Use the real binary name, e.g. `cargo test -p sf-map --test route1_parity`.

Bit me on the bossH→1_4 rewire (2026-07-08): pushed a green-looking commit that
actually left `level1_4_matches_c` failing because only the .bin was blessed.
Relevant to the pending [[overhaul-phase2-status]] map re-wires (bossBrob's
MAP1_6A un-stub will hit the same thing).
