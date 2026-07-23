# Native port architecture

The shipping Rust implementation is a source-level port, not an emulator.
`sf-oracle` may model original hardware in any form needed to produce reliable
reference results; none of that machinery may be reachable from `sf-app`.

## Required boundaries

- Native game state consists only of domain structs and typed collections.
  The shipping game API has no generic `Memory` object, byte arena,
  address-based field access, or independently mapped source-machine regions.
  `GameVars` is split into typed records that preserve the source game's
  conceptual layouts: shared game variables, map variables, strategy
  variables, path-program variables, and enemy-path variables.
- Source bank/address values may exist in generated data and extraction tools.
  Native systems consume decoded IDs, offsets, enums, and typed records rather
  than resolving source addresses during gameplay.
- Native code does not execute original CPU or graphics-coprocessor programs.
  Compatibility execution is verification-only, behind the `oracle-bridge`
  feature, and release dependencies must disable that feature.
- Native code does not name or model processor registers. Oracle code and
  disassembly tools are exempt.
- Shipping audio is a typed PCM mixer over music, engine, ambience, positional,
  and effect channels. Positional loops are selected from typed object fields,
  listener orientation, distance bands, and stereo-position enums; they do not
  read a generic byte arena or reproduce source-machine addressing. The
  original sound-processor program is available only through the `oracle-audio`
  feature to render and compare certified PCM assets offline; `sf-app` neither
  enables nor depends on that execution path. Missing native SF1 or SF2 audio
  assets fail startup instead of silently activating an emulator or placeholder.
- Every nontrivial numeric value in handwritten port code has a meaningful
  constant, enum variant, or typed newtype. Decimal is the default notation.
  Hexadecimal is reserved for values whose meaning is inherently bit-oriented
  or encoded: masks, packed colors, byte signatures, and source-file evidence.
- Comments may cite a source routine or encoded address as provenance, but the
  implementation must explain the gameplay meaning and must not transcribe
  register choreography.

## Retained program-data boundary

The SF1 map and path catalogs are still imported as compact programs. Encoded
variable operands are decoded into named fields only in
`sf-game/src/vars.rs`, `sf-game/src/game.rs`, and
`sf-strat/src/path_adapter.rs`. No ordinary gameplay routine may call the
encoded-operand accessors.

Native path callbacks do not inspect embedded processor instructions. The
catalog builder emits `(action, continuation)` metadata and the interpreter
calls a named Rust callback before continuing at the generated target. Native
strategy handles likewise use `StratRef::Native`; source addresses are kept
only for imported `P_SETSTRAT` records that still require lookup.

The map restart record stores a program offset plus typed background and
palette state. It intentionally has no bank or segment field.

## Verification

Run from the repository root:

```text
nix develop --command python3 tools/check_native_architecture.py
```

This check guards the dependency boundary, encoded-operand confinement,
byte-arena APIs, segmented map state, and processor-register vocabulary in
shipping source. Normal unit, route, replay, visual, and audio tests remain
responsible for behavior.
