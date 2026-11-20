---
title: Page turns
description: page_stack(at, content) gives one room to many pages; the page left behind is frozen and carried out.
about: crates/ui/rikka-widgets/src/navigation/page_stack.rs
tags: [ui, animation]
---

- `page_stack(at, content)` is used by sidebars, drill-ins, tab windows and opened folders.
- `at: PageKey` is a `usize` or an `(outer, inner)` pair. A larger spot enters from the right, a smaller one from the left, so no direction is passed in.
- `.named(key)` marks page identity when its spot can move (reordered tabs or rows).
- A page is a room: it lays out like `container`, never imposes its content's height, and grows its content unless the content asked for a size.
- The outgoing page is frozen via `Widget::keeps_children`, keeping content, scroll and measurements. It stands absolutely at its last size and is `INERT` (no pointer, focus or ids).
- `keeps_children` only means "no new children in this element" (the standing page says it too once its content is handed over). Freezing is `Widget::freezes_children`, said only by a leaving `Page`: the reconciler's `rebuild_walk` never descends into it, so components inside a leaving page are not re-viewed with the new state while the standing page stays live. A widget that freezes must never thaw, since what went stale inside is not walked again.
- At most three pages leave at once. The room clips across the travel axis for the whole turn.
- Opacity and travel are separate retargetable channels, so fast turning never jumps.
- The first page ever shown rises and fades in. `Env::carried` stops nested rooms from arriving twice; pages and any element with a `.transition` set it for their subtree.
- A finished departure rebuilds only the pager.
- `split_view(..).shows(at)` drives the page turn for every navigation list.
