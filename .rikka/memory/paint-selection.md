---
title: Paint selections and paint bucket
description: Coverage-span Region, Shape/Flood/Modify edits in paint-document, and the GUI selection tools that only order them.
about: apps/general/paint/crates/paint-document
tags: [paint, selection]
---

- `Region` stores per-row sorted runs of `Span { start, end, coverage: u8 }` (255 = fully selected, adjacent equal runs coalesced, zero dropped), built through the crate-private `region::Builder` (`push`, `end_row`, `finish`).
- `bounds()` and `outline()` (marching ants) follow the ≥50% `footprint()`; `extent()` covers any coverage; `mask(w, h)` hands the coverage to `imaging::Mask::coverage`, so `Fill`, `Erase`, `Develop` steps, strokes and crops honour soft edges.
- `Combine` acts per pixel: union = max, intersect = min, subtract = min(a, 255−b), xor = max of the two differences.
- Geometry lives in `shape.rs`: `Shape::{rectangle, ellipse, polygon(subpaths, FillRule), lasso}.anti_aliased(bool)` → `region(canvas)`; anti-aliasing is 16 sub-scanlines with exact horizontal coverage, hard edges take pixel centres.
- `flood.rs`: `Flood::at(x, y).tolerance(0..1).mode(FloodMode).source(Source).anti_aliased(bool)`; colours compare premultiplied RGBA, matching when the RMS difference ≤ tolerance² (paint.net's curve); anti-aliasing keeps the hard footprint.
- `modify.rs`: `Region::modified(Modify::{Expand, Contract, Feather, Border, Smooth}, canvas)`; expand is a disc dilation on spans, contract dilates the inverse so canvas edges stay, feather is a σ = radius/2 blur on a `raster::Raster`, smooth is a box majority.
- Edits: `SelectShape { shape, combine }`, `MagicWand { flood, combine }`, `SelectLayer { layer, combine }` (alpha becomes coverage), `ModifySelection(Modify)`, `MoveSelection { dx, dy }`, `Bucket { layer, flood, color }` (flood fenced by and intersected with the selection, applied as a masked `Fill`, `Action::Bucket`); every selection edit records `Action::Select(SelectionAction::…)`. Their `Document` methods are in `document/selecting.rs`.
- GUI (`canvas/selecting.rs`, `canvas/path.rs`): marquees preview through `Shape::region` and order `SelectShape`; lassos draw a dotted path and order on release, Enter, double-click or a click on the first corner; the wand and bucket only order edits; a plain drag inside the selection or arrow keys (`Command::Nudge`) shift the shown outline (`Selection::displace`) and order `MoveSelection`.
- Modify dialogs are `session/modify.rs` (`Kind::Modify(Modification)`) in the Edit ▸ Modify menu; per-tool options (`smoothing`, `wand`/`bucket` `Flooding`) live in `options::Options`.
