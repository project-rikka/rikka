---
title: Desktop widgets (desk) architecture
description: Per-wallpaper widget layouts: desk-layout crate, Board/Edges/DeskScene, scene morph, backdrop, overview, editor crate.
about: system/rikka-shell/crates/shell-desk
tags: [shell, desk]
---

- `system/rikka-shell/crates/desk-layout` is the pure, tested core: `Widget`/`Content`/`Kind`/`Size` (fixed steps, `Kind::sizes()`), `Area` (proportional `Spot` in the room inset by `MARGIN`), `Snapper` (smart guides), `vacant`/`first_vacant`, `pair` (morph matching), `Projection`, `Undo`, and `Desks` (TOML document, one layout per wallpaper id).
- `Board` holds `Desks`, persists `$XDG_CONFIG_HOME/rikka/desk.toml` through `shell_kit::StateFile` (atomic replace) and is `Observed`; an unreadable or partly skipped file is copied aside (`.unreadable`/`.skipped`) before anything is written.
- The first time a wallpaper is shown it gets a copy of the previous wallpaper's layout (`Board::show`, fed by the `rikka.appearance/wallpaper` setting); `foreseen` previews that copy.
- `Edges` is the one follower of the taskbar standing: it gives the desk `Area` (taskbar strip removed) and the editor's chrome mock. `DeskScene { seam, board, edges, feeds, focus }` is the handle Backdrop, Overview and the editor share; `DeskScene::edit`/`offer_menu` open `rikka.desk` / `rikka.desk-menu` (passing the pointed widget through `focus`) and `DeskScene::launch` starts programs through the shell's `rikka.launch` service.
- `shell_kit::toplevels::Toplevels` is the shared per-output window list (overview cards, editor fade-out); `shell_wallpaper::stage` holds the staging geometry both overlays use. The overview and editor backdrops follow their wallpaper through change streams (`Desktop::changes`, `Media::changes` until a video is ready, a `LINGER` timer for the cross-fade), never by polling.
- Widgets live in their own transparent per-output Bottom layer (`desk_surface.rs` `DeskSurface`, `SurfaceDesc::desktop()`, `KeyboardMode::OnDemand` for notes, whole-surface pointer for long-press and menu), so their panes are compositor glass over the wallpaper and window captures never show them. Which wallpaper's layout to show comes from the wallpaper feature's `ShownOn`.
- `scene::scene(Scene, &Live)` is one output-sized element; it morphs (pair, glide, fade) when `Scene.wallpaper` changes and glides on edits; `scene::desktop` adds long-press (edit) and secondary click (menu). The backdrop switches the scene's wallpaper only when the wallpaper crossing begins, so both run together.
- Scaled copies (overview cards, editor card) use `shell_kit::framed`: content laid out at output size and painted under `set_subtree_scale`; place the framed node over the whole surface because culling uses unscaled bounds; framed content is inert.
- Framed children live in unscaled space starting at the placed rect's origin, so a children clip set in `place` must be `Rect(placed.origin, natural)` with radius divided by the factor; anything else clips and culls widgets near the right/bottom.
- The editor overlay uses namespace `surfaces::DESK`, which the compositor treats like the overview (`surfaces::hides_windows`); the editor draws the windows itself, fading with the card. Shortcut `Shortcut::EditDesk` (Super+W).
- Sub-notes: widget contents in `desk-widgets.md`, editor in `shell-desk-editor.md`.
