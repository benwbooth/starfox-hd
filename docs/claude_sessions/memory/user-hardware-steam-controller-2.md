---
name: user-hardware-steam-controller-2
description: Ben uses a Steam Controller 2 — needs newer/dev SDL (likely SDL3) for proper support
metadata: 
  node_type: memory
  type: user
  originSessionId: 8f7b8292-fbc0-48f8-8506-8b6a0949123b
---

Ben's gamepad is a **Steam Controller 2** (noted 2026-07-01). Old SDL2 (nixpkgs sdl2-compat) detects it as "Steam Controller" but inputs don't register in the Star Fox HD port.

**How to apply:** for gamepad work, prefer SDL3 (or a dev SDL2 with current controller db/hidapi) — the Rust port's sf-app input layer should use SDL3 bindings. Also consider Steam Input / lizard-mode interference when debugging. Gamepad support is scheduled post-RIIR per Ben. [[overhaul-phase2-status]]
