---
title: Toolbars with priority overflow (rikka-widgets toolbar)
description: toolbar()/tool_group/tool_item/tool_button: groups never shrink, collapse by ToolPriority into a ⋯ menu.
about: crates/ui/rikka-widgets/src/navigation/toolbar
tags: [ui, widgets]
---

- Every app toolbar is `toolbar()` (Animate bar and node editor header, Cut timeline/monitor/status, Files window and chooser, Packages, Remote, Browser reader). A plain `row!` of controls overlaps when narrow; never build toolbars that way.
- Entries: `.group(tool_group(items).priority(..).gap(..))`, `.spacer()`, `.flexible(view, least)` (grows, never below `least`, never overflows). `.divided()` draws a hairline between adjacent visible groups not split by a spacer or flexible entry.
- Groups keep their natural width (slots are `fixed`). When the room is short, groups collapse whole, `ToolPriority::Low` first, then `Normal`, then `High`, rightmost first within a level; `Pinned` never collapses (the toolbar `CLIPS`).
- Collapsed groups contribute their items' `tool_item(..).overflow([MenuItem])` forms to the trailing ⋯ menu (separated per group); items without one just vanish (status text). No overflow items, no ⋯ button.
- `tool_button(symbol, label)` is the standard icon button (focusless, Small/18 px by default, Medium/24 px via `.control_size`), with `.on_press/.enabled/.checked/.variant/.shortcut(chord)`; it is both a `View` and a `ToolItem` whose overflow form is the matching menu row. Apps wrap it per command table (`Command::tool` in Animate and Cut).
- Overflow forms of other controls: combos and segmented controls use `menu_item(label).choices(labels, chosen, on_select)`, sliders `menu_slider(label, value, range).step(..).on_change(..)` (drag or ←/→ inside any menu host).
- Mechanics: `Toolbar` is `ARRANGES`; `arranged` measures every slot (hidden ones are placed out of flow past the right edge, `INERT`), runs `fit::hidden`, and rebuilds when the hidden set or room changes. Content elements are taken once from a `RefCell`; arrange-triggered rebuilds re-emit them as `Element::retained`.
- Anchored popups opened from toolbar buttons (Files sort/tools, Cut range shapes, node editor menus) sit beside the toolbar, not inside a group, so a collapsed group never makes them inert.
- Tests: `crates/ui/rikka-widgets/tests/toolbar.rs` (collapse order, overflow choosing, menu slider keys).
