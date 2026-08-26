# Automated retail-parity plan

## Goal

Deliver an automated, evidence-backed Star Fox 1 retail-parity release in the
modern flat-memory, register-free, typed Rust port. Build independent retail
and native deterministic replay adapters; cover menus, attract mode, training,
all routes and difficulties, bosses, deaths, pause/resume, continues, endings,
and scripted interaction variants; compare per-tick semantic state, stable
object identity and lifecycle, ordered gameplay/audio events, source-resolution
frame pixels, and PCM/audio routing. Fix the earliest true divergences,
including level-select music, Corneria intro corridor flicker, blue-tower
destruction spawning incorrect Arwings, and glowing-arch behavior, until the
corpus and reachable-edge coverage gates report zero unexplained differences.
Allow no masks, quarantines, state copying, RNG resynchronization, placeholders,
or visual compensations. Keep ROM emulation confined to `sf-oracle`; enforce
the native flat-memory/domain-struct architecture, no processor-register
vocabulary, named constants/enums, numeric-style rules, exact ROM roundtrip
checks, full workspace tests, and unattended runtime checks. Commit and push
each independently verified tranche. After SF1 certification, produce the
measured SF2 parity backlog and reuse the same verification pipeline.

## What counts as proof

A feature is complete only when a deterministic retail/native scenario reaches
the same checkpoint and all applicable comparison surfaces agree:

- semantic state: game mode, map phase, player state, camera, score, shields,
  inventory, progression, and deterministic random-stream position;
- object state: stable semantic identity, spawn/removal order, ownership,
  transform, collision, damage, and strategy phase;
- ordered events: weapons, hits, destruction, rewards, checkpoints, dialogue,
  music, and sound effects;
- video: source-resolution completed retail frames versus native frames at
  authored presentation times;
- audio: music selection and routing plus sample-exact PCM where the native
  asset is intended to be an oracle render.

The comparator must fail closed. Missing evidence, an unreached checkpoint, a
skipped oracle, a tolerated field, or an unexplained difference is a failure.
Native state must never be copied from the oracle to manufacture agreement.

## Scenario corpus

Scenarios are data manifests, not bespoke test programs. Each manifest pins the
ROM revision, initial conditions, controller input by retail refresh, expected
checkpoints, comparison surfaces, and termination condition. The corpus grows
in this order:

1. Boot, publisher sequence, title, control selection, level selection,
   briefing, attract mode, and training.
2. Corneria opening and normal flight, including corridor presentation, all
   four skill arches, tower and carrier destruction, player damage, death,
   pause/resume, and the stage boss.
3. Every route and difficulty, including alternate exits, bonus conditions,
   checkpoints, bosses, continues, game over, credits, and endings.
4. Interaction variants for every reachable object class: observe, evade,
   collide, damage once, destroy, leave behind, and interact with its children
   or reward when applicable.

Coverage is measured by source strategy/state transitions and authored map
events reached by the corpus. A passing route that never triggers an event does
not cover that event.

## Divergence workflow

For each scenario, capture both sides independently, align them on authored
retail refresh and semantic checkpoints, and report the first divergent field,
event, pixel region, or PCM sample. Fix only that earliest causal divergence,
turn it into a permanent regression, rerun all earlier gates, then commit and
push the verified tranche. Never tune later frames around an earlier mismatch.

## Current reported regressions

- Level-select music: source selection resolves to map track 5, cue 1, with
  authored spherical and flat-object zoom cues 11 and 13. The feature-gated
  audio test boots the retail SPC data and compares five seconds of each
  shipping WAV with independently rendered oracle PCM sample-for-sample.
- Corneria corridor: completed-retail-raster frame capture and the authored
  presentation timing table now guard the opening. The broader scenario still
  remains in the release corpus until all semantic, video, and event channels
  pass together.
- Blue tower: destruction enters the retail pillar-explosion chain. Typed
  explosion envelopes cannot alias catalog mesh 2 (the Arwing), and lifecycle
  tests cover the staggered explosion children and removal cadence.
- Glowing arches: a normal-controller-input integration scenario crosses all
  four skill checkpoints, creates and collects the twin-laser reward, and
  checks its pickup event. These Corneria arches are a four-checkpoint skill
  course; they are not the separate shield-restoring gate object.

These regressions prevent known failures from returning, but they do not by
themselves certify the whole game. Certification requires the complete corpus
and coverage gate above.

## Unattended gate

Run the complete current gate from the repository root:

```sh
./scripts/verify_retail_parity.sh
```

It fails if required retail/audio evidence is missing, then checks formatting,
the full Rust workspace, the feature-gated SPC oracle suite, native architecture
rules, and byte-exact reconstruction of both supported retail ROMs. As the
scenario corpus expands, its top-level test targets remain behind this command
so local and automated runs cannot accidentally exercise different gates.

## Release stop condition

Star Fox 1 is ready for the user's single final test only when all of the
following are true:

- every scenario manifest completes and every enabled comparison surface has
  zero unexplained differences;
- every reachable source strategy transition and authored map event has a
  recorded corpus hit, or is proven unreachable in the pinned retail revision;
- the native architecture and source-style checks pass without allowlisted
  shipping violations;
- the full workspace, real application smoke tests, SPC oracle tests, and exact
  ROM roundtrip checks pass from a clean checkout through the one-command gate;
- the resulting commit is pushed and the remote branch is verified at the same
  commit.

SF2 begins only after that SF1 gate is green. Its backlog is derived from the
same measured source-transition and scenario coverage reports rather than a
subjective completion percentage.
