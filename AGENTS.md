Work autonomously until the task is complete. Don't ask me to continue unless you are blocked by missing info or a hard permission boundary.

# Star Fox HD — Rust-only

The project is a pure-Rust reimplementation. The legacy C/C++ tree (`src/`,
`CMakeLists.txt`) has been removed; the Rust workspace under `rust/` is the
whole build. The SPC-700 audio core is pure Rust (`sf-spc`), so there is no
C/C++ compilation anywhere in the default build.

## Build & run (inside the nix devshell)

    nix develop --command bash -c "cd rust && cargo build"
    nix develop --command bash -c "cd rust && cargo test --workspace"
    nix develop --command ./scripts/run.sh          # launch from the repo root

`scripts/run.sh` runs the binary from the repo root so `starfox.ini` and
`data/` resolve cwd-relative — no CMake / asset-staging step. Shaders are
embedded (byte-equal to `rust/sf-render/shaders/*.glsl`); pass
`--shader-dir rust/sf-render/shaders` to load them from disk.

## Layout

- `rust/` — cargo workspace (engine, renderer, audio, app binary `sf-app`).
- `rust/sf-render/shaders/` — canonical GLSL.
- `data/` — extracted game assets (user-owned, gitignored, not moved).
- `reference/` — original SNES ASM disassembly (RE reference).
- `tools/` — Python codegen (e.g. `shape_compiler.py` -> `rust/sf-render/src/shape_data.rs`).
