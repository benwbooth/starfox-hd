#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 "$project_root/tools/rom_roundtrip.py" \
  --manifest "$project_root/tools/roundtrip/manifests/sf1-us-rev2.json" \
  --rom "$project_root/Star Fox (USA) (Rev 2).sfc"

python3 "$project_root/tools/rom_roundtrip.py" \
  --manifest "$project_root/tools/roundtrip/manifests/sf2-us-europe.json" \
  --rom "$project_root/Star Fox 2 (USA, Europe).sfc"
