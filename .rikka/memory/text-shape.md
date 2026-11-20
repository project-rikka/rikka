---
title: Shaping engine (rikka-text-shape) and oracle bench
description: Reference-equivalent shaping incl. Indic, Khmer, Myanmar and USE; benchmarks/text checks it against the reference.
about: crates/text/rikka-text-shape
tags: [text, shaping]
---

- Shapes one run (one face, script, direction, language, features, location). It depends on `rikka-text-font` and `rikka-text-unicode`.
- `ShapeFace::new(font: Font<'static>, location)` returns an `Arc` (`Send + Sync`) that owns its font and the whole `FontContext` (`face/font_context.rs`): table slices, charmap, lookup accelerators, cmap and advance caches, GDEF classes, variation scalars. Keep one per face and location; tests use `rikka_test_fonts::loaded_font` for `'static` bytes.
- `face.plan(&SegmentProperties, &Features)` returns a `ShapePlan` (face + compiled core) from the face's one plan cache (`plan/plan_cache.rs`: eight lock-free slots, then a 56-entry overflow). Callers keep no plan cache of their own. `plan.is_equivalent(&other)` is true when both shape identically: same face, the script-neutral default shaper, same direction and language, same lookups and masks.
- Hot path: `plan.shape(&mut buffer)` per run, no per-run setup. Reuse the `Buffer`; steady state allocates nothing.
- `Buffer` clusters are UTF-8 byte offsets (`push_str(text, base)`). Output is `GlyphInfo` (glyph, cluster, unsafe-to-break/concat) and `GlyphPosition`, in integer-rounded font units like the reference shaper.
- Complex shapers implement `ComplexShaper` in `src/shapers/`. Exactly one table, `shapers/registry.rs`, routes scripts to shapers, matching harfrust's routing (Indic `*3` tags and every USE script go to `universal`).
- `shapers/syllabic/` is shared by Indic, Khmer, Myanmar and USE: grammars are `Pattern` combinators compiled once into a longest-match DFA (`SyllableMachine`), plus dotted-circle insertion and vowel constraints.
- Indic/Khmer/Myanmar categories come from `indic/properties.rs`, USE categories from `universal/categories.rs` (UCD + MS-USE overrides in `overrides.rs`); both are lazily paged `CodepointTable`s derived the way the reference generators do.
- Variable advances and device deltas use rikka-text-font's 16.16 readers (HVAR, else gvar phantom points), rounding like fontations; a varied face caches region scalars and per-glyph advances.
- Lookups compile lazily per face into `LookupAccel` (`face/lookup_accel.rs`): subtable locations (no reparsing of the lookup list), a glyph → (subtable, coverage index) candidate index used for every lookup, deduplicated bitset coverage sets for context format 3 and GDEF mark sets, first-key rule buckets for chain formats 1/2, and coverage maps for mark/cursive targets. Apply paths read only these, never Coverage or ClassDef parsing.
- Context match positions are frame-local; lookups whose feature mask no glyph carries are skipped.
- `benchmarks/text` (crate `text-bench`): `tests/oracle.rs` and `tests/random_oracle.rs` require output identical to harfrust; a mismatch is a rikka-text-shape bug unless `harfrust_divergence.rs` classifies it as a proven harfrust bug (each has a rikka-text-shape test asserting reference behaviour). `tests/references.rs` compares the benchmark texts with the reference shaper itself (a divergence table lists known differences).
- Speed comparisons are the `benchmarks/text` divan suites (`shaping`, `raster`, `paragraphs`, `fonts`, `unicode`), run with `bazel run --config=bench //benchmarks/text:<suite>`.
- Bench font names like `NotoSans.ttf~HVAR` load the installed font with that table removed.
- harfrust and the other comparators are for `benchmarks/` only; no `crates/text` crate may use them.
