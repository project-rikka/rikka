---
title: Mesh robustness sweeps (rikka-test-meshes)
description: Shared awkward-mesh corpus, seeded operator/chain sweeps and reference-divergence rules for degenerate input.
about: crates/model/rikka-test-meshes
tags: [testing, modeling]
---

- `crates/model/rikka-test-meshes` (testonly, visible to `crates/model`) holds `corpus()` (empty, loose, wires, ico triangles, triangulated blob, mixed tri/quad/n-gon, concave/non-planar n-gons, fin, bow-tie+wires+loose, doubled faces, faces sharing all verts, open grid, degenerate, coincident, every-data-layer mesh) and `sample_fixtures()` (300-face patches of miku.pmx, avatar.vrm, fox.glb loaded through the codec importers; files come in as library `data`).
- `sweep_edit(name, fixtures, cases, &[(name, EditOperator)])` runs each `fn(&mut EditMesh, &mut Rng) -> Result<(), String>` over seeded `Selection`s (mode × empty/single/partial/all × hidden), accepting `Err` as a note and checking `check()` (validate, layer lengths via `to_polymesh().validate()`, no NaN from finite input). `sweep_chains` runs random operator sequences; `replay_chain` + `python_dump` reproduce a failing chain step and dump it for the reference script.
- `Sweep` catches panics with location (also echoed to stderr, since a panic that kills the case process would otherwise vanish), prints slow cases (>1 s), aborts naming the case after 20 s (watchdog), and `tolerate(prefix)` downgrades a known failure to a note. `Amount::pick`/`choose` pick empty/single/partial/all subsets of any list.
- Each operator crate keeps its sweep in `tests/robustness.rs`; the modifier sweep compares `evaluate_cached` (cold and after nudged positions) with `evaluate`.
- Rule for degenerate input: where the reference's maths overflows (UB or NaN) the port follows the reference's observable result when it is finite (e.g. data-transfer ray counts emulate the C cast to INT_MIN) and otherwise takes the nearest defined branch (bevel `line_line` treats non-finite meets as parallel).
