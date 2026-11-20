---
title: Shell screenshot and recording editors
description: markup draws over a just-taken screenshot; trim cuts a recording losslessly; both reopen from notification cards.
about: system/rikka-shell/crates
tags: [shell, capture]
---

- Both are `shell_kit::floating` overlays with presence methods plus `edit`. Pressing the notification card (stack or history) calls `edit`.
- Every blocking task carries the opening it was for, so late results for a replaced file are dropped.
- Neither editor ever asks before closing: close and a newly arrived file both drop unsaved edits; only Save (Ctrl+S) writes.
- Markup (`rikka.markup`): `Editing` hands out the errand token and keeps the newest shot in memory only when it was not saved to a file; a saved shot or a stale token rereads the file. Capture always names a file, even for clipboard-only shots.
- Markup saves over the file at the original quality (`Opening`) and copies the PNG.
- Markup's `Canvas` widget maps window points to picture pixels and shows one computed marks layer at display resolution. Input only marks it stale, and painting rasterizes once per frame.
- The live mark stays in widget state until release. Marks live in `rikka-image-edit`; the canvas shows them as a GPU `marks_layer` through `ImageSource::Computed`, and saving or copying develops them into the picture on the GPU through `imaging::shared`. `Choices` (tool, colour, width) outlive the surface.
- Trim (`rikka.trim`): the errand carries only the path. `capture::recorder` attaches it via `trim::errand`.
- `Cut` models length, kept span, minimum length and keyframe-aligned start. The `Timeline` widget paints and reports; the component snaps, clamps and seeks.
- `Keyframes` is scanned once per opening, and the timeline stays inert until it lands.
- `Preview` wraps a `rikka_movie::Movie`; the editor follows `Movie::on_change` (a keyed stream per opening, no tick) to stop at the span end, measure the length once and catch failures.
- `writing::write` writes a hidden sibling and renames it over the original, then reopens what was written.
