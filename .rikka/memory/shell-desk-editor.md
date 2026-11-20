---
title: Desk widget editor and desktop menu
description: shell-desk-editor (Super+W edit mode: editor_canvas, overlay, framed card scene, panel) and the desk's menu.
about: system/rikka-shell/crates/shell-desk-editor
tags: [shell, desk]
---

- `rikka.desk` opens one exclusive overlay per output, namespace `surfaces::DESK`. The `DeskEditor` feature (`system/rikka-shell/crates/shell-desk-editor`, built from `Desk::desk_scene()`) serves `rikka.desk` and its `EditDesk` shortcut; the address is `shell_desk::CAPABILITY`, so `DeskScene::edit` needs no editor. `menu::install` stays in the desk crate (`feature.rs`).
- Look follows the overview: no backdrop of its own: a `DesktopFrost` frosts the whole desktop under the taskbar (opacity = spread, set in `editor_canvas/paint.rs` through `Sheet::frost`), a centred card (wallpaper, widgets, taskbar mock), a wallpaper thumbnail strip on top and a glass side panel (gallery or inspector) on the right. Spread 0 equals the desktop; exit is the `DEPARTING` tween.
- `stage.rs` (`Editor` State) is the only session reader. `view.rs` builds the root `editor_canvas` widget with three absolute layers: the card scene component (`card.rs`, `shell_kit::framed` over `scene::scene` in `Mode::Shown`), the paint-only `overlay` (mock chrome, rings, toolbar, guides, carry ghost) and the `panel`.
- `editor_canvas/` owns springs, hit tests and all pointer/keyboard editing; `editor_canvas/frame.rs` is pure geometry. It publishes the card rect, lens and spread as a `Shot` in the per-output `Session`; framed and overlay read it in the same paint pass and request frames while it moves.
- `Session` (per output, `Observed`) carries the drag draft (re-runs only the card component), marks (overlay repaint), toolbar hit rects and the gallery press offer; drags never re-view the window.
- `Editing` (shared) is the only writer: every committed edit goes through `Board::change` on the showing wallpaper and records `History` (undo of `(wallpaper, Layout)`, consecutive nudges coalesce; undo switches back to that wallpaper).
- Pure placement math lives in `desk-layout`: `Projection` (desk ↔ card), `first_vacant` (gallery auto-placement), `Snapper`, `vacant`, `Undo::earlier/later`.
- `WallpaperLoader` loads and cross-fades the card wallpaper; `Scene.wallpaper` is the id whose picture is shown, and editing waits until it equals the showing one.
- `menu/` is `rikka.desk-menu`: an aimed popover menu (edit, remove widget, change wallpaper, settings) that takes `DeskScene::focus` and hands it back through `DeskScene::edit`. Change wallpaper launches Settings at `Spot::Wallpaper` and settings at `Section::Appearance` (`rikka_session::settings_app::Link`) through `DeskScene::launch(argv)`; the editor's strip stays the in-place wallpaper switcher.
