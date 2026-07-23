#!/usr/bin/env python3
"""Extract Mesen's embedded rendered-frame preview from a savestate.

This is oracle tooling only.  It calls the same native API used by Mesen's
savestate picker and does not advance the emulated machine.
"""

from __future__ import annotations

import argparse
import ctypes
from pathlib import Path


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
PNG_END_CHUNK = b"IEND\xaeB`\x82"
PREVIEW_BUFFER_SIZE = 512 * 478 * 4


def extract_preview(core_path: Path, savestate_path: Path) -> bytes:
    core = ctypes.CDLL(str(core_path))
    core.InitDll.argtypes = []
    core.InitDll.restype = None
    core.GetSaveStatePreview.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_uint8)]
    core.GetSaveStatePreview.restype = ctypes.c_int32

    core.InitDll()
    buffer = (ctypes.c_uint8 * PREVIEW_BUFFER_SIZE)()
    result = core.GetSaveStatePreview(str(savestate_path).encode(), buffer)
    if result <= 0:
        raise RuntimeError(f"Mesen rejected savestate preview: {result}")

    # MesenCE 2.2.1 reports the uncompressed frame byte count here even though
    # the buffer contains PNG data.  Locate the PNG terminator explicitly.
    raw = bytes(buffer)
    if not raw.startswith(PNG_SIGNATURE):
        raise RuntimeError("Mesen preview buffer does not begin with a PNG signature")
    end = raw.find(PNG_END_CHUNK)
    if end < 0:
        raise RuntimeError("Mesen preview buffer has no PNG end chunk")
    return raw[: end + len(PNG_END_CHUNK)]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("savestate", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--mesen-core", required=True, type=Path)
    args = parser.parse_args()

    args.output.write_bytes(
        extract_preview(args.mesen_core.resolve(), args.savestate.resolve())
    )
    print(f"wrote {args.output} ({args.output.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
