#!/usr/bin/env python3
"""Run all SF2 data extractors (SF2_RECON.md phase 1).

    nix develop --command python3 tools/sf2/extract.py

Writes the generated modules under rust/sf2-data/src, including exact colors,
shapes, polygon textures, map/path data, text, and audio. Audio also writes raw
blobs under data/sf2/snd/ (gitignored). See each extract_*.py for details.
"""

from __future__ import annotations

import sys

import extract_audio
import extract_collision
import extract_colors
import extract_lighting
import extract_map
import extract_palettes
import extract_path
import extract_shapes
import extract_text
import extract_textures
from rom import load_rom


def main() -> int:
    d = load_rom()
    print("SF2 data extraction:")
    extract_audio.extract(d)
    extract_collision.extract(d)
    extract_colors.extract(d)
    extract_lighting.extract(d)
    extract_text.extract(d)
    extract_shapes.extract(d)
    extract_palettes.extract(d)
    extract_textures.extract(d)
    extract_map.extract(d)
    extract_path.extract(d)
    print("done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
