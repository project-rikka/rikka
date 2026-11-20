---
title: rikka-ui layout engine and sizing rules
description: rikka-ui's four-sweep layout, relayout boundaries, min sizes, text wrapping, Shrinkable leaves, scroller pitfalls.
about: crates/ui/rikka-ui/src/runtime/layout
tags: [ui, layout]
---

- `runtime/layout/` owns its own arena (`Layouts`, `Node` in `Instance::node`) solved in four sweeps: min/max-content widths ↑, widths ↓, heights from resolved widths ↑, heights, positions and pixel rounding ↓. Only dirty paths or changed sizes are revisited. `Axis` names every direction. The public `LayoutTree` (`container`, `leaf`, `set_children`, `set_layout`, `set_measure`, `solve`, `rect`) runs the same engine standalone.
- Relayout boundaries (`layout/boundary.rs`): a node with px width and height, no inset and a non-scrolling parent stops dirtiness climbing (`mark` climbs from the parent, `mark_contents` stops at a boundary). Queued boundaries are solved in place with their last offered sizes; if the parent-visible facade changed, the parent is marked instead. Callers gating on dirtiness use `Layouts::needs_solve` and also walk `boundaries()`.
- `Layouts` caches each node's `Edges` and an out-of-flow flag beside its style (`adopt_style`). A text leaf whose measure changed keeps its `Leaf` (`Leaf::respec`); `Leaf::sized` reuses a laid width that `serves` the asked one.
- A container's automatic minimum main size is 0 (its padding), so panes shrink to fit without `min_height(0)`; text and other leaves keep their min-content. Rows that must keep height say `shrink(0)`.
- Text wraps `Wrap::WordOrGlyph` by default (CSS `overflow-wrap: break-word`): a word wider than its row breaks between graphemes, while a wrapping leaf's min-content stays its widest word, so text inside a container never overflows it.
- A self-painted leaf that can give way reports `MeasureSpec::Shrinkable { least, natural }` instead of a hard `min_width`, so hugging parents get its natural width and narrow ones never see it overflow. `segmented` (and combo group tabs) use it: equal segments when roomy, then widths by label, then the longest labels elide first with a tooltip.
- A scroller never shrinks its children along its scrolling axis: flex lines resolve against at least their natural length (`flex::room`) and grid bands take their full content size, so shrinkable content (a `fits_text` editor) keeps its height inside `scrollable` and lazy lists.
- The scroll bar's lane is reserved only on overflow, solved inside the layout (`settle_lane`), for every sizing, including `HUG` capped by `max_height`. `PaintCtx::room`/`EventCtx::room` exclude it.
- Pitfall: collections and `scrollable` need a given height (`fill()`, `grow`, or a measure); never nest them in `scrollable`.
- Pitfall: `container` lays out as a row, so a child without `fill_width()` hugs its content; `markdown(..)` and other `deferred` content then collapse to zero width and never build. Always give them `.fill_width()`.
- `Widget::caret` answers relative to the widget's origin and `Ui::caret` adds the node's bounds. `Mark::Relayout` (`Observers::reshaped`) re-resolves the widget's `layout()` as well as its measure.
- `PaintCtx::reveal(rect)` travels only up the requester's own ancestors until a scroller's `Widget::reveal` takes it; `reveal_with_parent_start` first stretches the rect up to the parent's top. Scrollers bring it in by `bar::reveal`, CSSOM `nearest`.
- `container(..).resizing()` / `resizing_upward()` spring a box to its content's height; `animated_position(child)` carries a child to its new layout spot.
- `collapsible(Option<content>)` (`blocks/collapsible.rs`) opens and closes a section by `GLIDE`: content slides out from under the edge above and, when it turns `None`, stays frozen (`keeps_children`, inert) while it tucks back, then is dropped. `.reveals_on_open()` makes an opening glide reveal the parent's top through the final content each frame; the rikka-chat Bash card uses it.
- Opacity (`.opacity`, fade transitions, `PaintCtx::with_opacity`) is group opacity through a renderer layer; nested fades multiply.
