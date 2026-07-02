#!/usr/bin/env python3
"""Recursive-descent (flow-following) 65816 disassembler for SF2 host banks.

Walks code starting from the reset vector and other seeds, following branches,
JSR/JSL calls and JMP targets, tracking M/X flag state along each path so
immediate widths decode correctly. Produces an annotated listing plus a symbol
map. Also emits a linear disassembly window on request for table/region study.
"""
from __future__ import annotations
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cpu65816 as c

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
ROM = os.path.join(REPO, "Star Fox 2 (USA, Europe).sfc")


def load():
    with open(ROM, "rb") as f:
        return f.read()


class FlowDisasm:
    def __init__(self, rom):
        self.rom = rom
        self.insns = {}          # file_off -> Insn
        self.labels = {}         # cpu addr -> label
        self.call_targets = set()
        self.jump_targets = set()
        self.data_refs = set()   # abs data addresses referenced
        self.visited = set()

    def add_label(self, cpu, name):
        self.labels.setdefault(cpu, name)

    def run(self, seeds):
        # seeds: list of (file_off, m, x, name)
        work = []
        for off, m, x, name in seeds:
            if name:
                self.add_label(c.file_to_cpu(off), name)
            work.append((off, m, x))
        while work:
            off, m, x = work.pop()
            self._walk(off, m, x, work)

    def _walk(self, off, m, x, work):
        rom = self.rom
        limit = len(rom)
        while True:
            if off < 0 or off + 1 > limit:
                return
            if off in self.visited:
                # merge point: stop (already decoded from some path)
                return
            try:
                ins = c.decode_one(rom, off, m, x)
            except (KeyError, IndexError):
                return
            self.visited.add(off)
            self.insns[off] = ins
            m, x = c.update_flags(ins, m, x)

            mnem = ins.mnem
            # record targets
            if ins.target is not None:
                tf = c.cpu_to_file(ins.target)
                if mnem in c.CALLS:
                    self.call_targets.add(ins.target)
                    if tf is not None:
                        work.append((tf, m, x))
                elif mnem in c.BRANCHES:
                    self.jump_targets.add(ins.target)
                    if tf is not None:
                        work.append((tf, m, x))
                elif mnem in ("JMP","JML") and ins.mode in (c.ABS, c.ABL):
                    self.jump_targets.add(ins.target)
                    if tf is not None:
                        work.append((tf, m, x))

            # linear flow termination
            if mnem in ("RTS","RTL","RTI","STP","BRA","BRL"):
                # unconditional branch: follow its target only (already queued), stop fallthrough
                return
            if mnem in ("JMP","JML"):
                return
            off += ins.length

    def autolabel(self):
        for cpu in sorted(self.call_targets):
            self.add_label(cpu, f"sub_{cpu:06X}")
        for cpu in sorted(self.jump_targets):
            self.add_label(cpu, f"loc_{cpu:06X}")

    def listing(self, lo=None, hi=None):
        out = []
        for off in sorted(self.insns):
            if lo is not None and off < lo: continue
            if hi is not None and off >= hi: continue
            ins = self.insns[off]
            if ins.cpu in self.labels:
                out.append("")
                out.append(f"; ---- {self.labels[ins.cpu]} ----")
            out.append(c.fmt_insn(ins, self.labels))
        return "\n".join(out)


def linear(rom, off, count, m=1, x=1, labels=None):
    """Linear (non-flow) disassembly of `count` instructions from off."""
    out = []
    for _ in range(count):
        try:
            ins = c.decode_one(rom, off, m, x)
        except (KeyError, IndexError):
            break
        m, x = c.update_flags(ins, m, x)
        out.append(c.fmt_insn(ins, labels))
        off += ins.length
    return "\n".join(out)


if __name__ == "__main__":
    rom = load()
    # Reset vector at file 0x7FFC (emu) -> $FBB8 -> file 0x7BB8
    reset_cpu = c.u16 = rom[0x7FFC] | (rom[0x7FFD] << 8)
    reset_file = c.cpu_to_file(0x00 << 16 | reset_cpu)
    print(f"; reset vector = ${reset_cpu:04X} -> file {reset_file:06X}")
    fd = FlowDisasm(rom)
    fd.run([(reset_file, 1, 1, "RESET")])
    fd.autolabel()
    print(f"; decoded {len(fd.insns)} instructions, {len(fd.call_targets)} call targets")
    print(fd.listing())
