#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

required_files=(
  "$project_root/Star Fox (USA) (Rev 2).sfc"
  "$project_root/Star Fox 2 (USA, Europe).sfc"
  "$project_root/data/snd/SGSOUND0.BIN"
  "$project_root/data/native_audio/music/track_05_cue_01.wav"
  "$project_root/data/native_audio/music/track_05_cue_0B.wav"
  "$project_root/data/native_audio/music/track_05_cue_0D.wav"
)

for required_file in "${required_files[@]}"; do
  if [[ ! -f "$required_file" ]]; then
    echo "retail parity prerequisite is missing: $required_file" >&2
    exit 1
  fi
done

cd "$project_root"
nix develop --command bash -c '
  set -euo pipefail

  python3 -m unittest discover -s tools/sf1 -p "test_*.py"
  python3 tools/sf1/verify_corneria_semantic_oracle.py \
    --mesen-bin "${MESEN_BIN:?Set MESEN_BIN to the independent Mesen executable}" \
    --timeout 600

  cd rust
  cargo fmt --all -- --check
  cargo test --workspace
  cargo test -p sf-audio --features oracle-audio --test boot_tracks

  cd ..
  python3 tools/check_native_architecture.py
  ./scripts/verify_rom_roundtrip.sh
'
