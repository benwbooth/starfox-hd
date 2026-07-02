#!/usr/bin/env python3
"""Run all SF2 data extractors (SF2_RECON.md phase 1).

    nix develop --command python3 tools/sf2/extract.py

Writes rust/sf2-data/src/{audio,colors,text,shape_data}.rs and, for audio, the
raw blobs under data/sf2/snd/ (gitignored). See each extract_*.py for details.
"""

from __future__ import annotations

import sys

import extract_audio
import extract_colors
import extract_shapes
import extract_text
from rom import load_rom


def main() -> int:
    d = load_rom()
    print("SF2 data extraction (phase 1, data-only):")
    extract_audio.extract(d)
    extract_colors.extract(d)
    extract_text.extract(d)
    extract_shapes.extract(d)
    print("done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
