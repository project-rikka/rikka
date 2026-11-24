---
title: Shell wallpaper library and backdrop
description: The shell owns the wallpaper folder, thumbnails and backdrop; everyone else names a wallpaper by id.
about: system/rikka-shell/crates/shell-wallpaper
tags: [wallpaper, shell]
---

- The library is `UserDirs::wallpapers()` (`Pictures/Wallpaper`). The shell creates and reads it but never writes pictures, and there is no `add`; Files' Set as wallpaper copies pictures in, and the watcher picks them up.
- The id is the file name. `Library::at_id` refuses anything but one plain component, and non-UTF-8 names are skipped. `shell_wallpaper::file_of(id)` gives the path.
- `rikka.wallpaper` (4.0): `list` (deferred to the `LibraryWatcher` thread), `refresh`, `show` (one `is_file`; unknown ids warn and refuse), `showing` (id + file), and `reserve`/`reserved`: a `Reserved` inset per edge (the opaque taskbar strip's edge only) that the wallpaper is laid out inside.
- `LibraryWatcher` holds a `rikka_xdg::Watch` via `Weak`. Announcements coalesce.
- Thumbnails live in the shared freedesktop cache through `rikka-thumbnails` (`x-large`, keyed by the file URI, valid while `Thumb::MTime` matches), so Files and the shell reuse each other's; made on one queue thread and never pruned by the shell.
- `asked` and `refused` stop repeated requests. Videos get `Preview::Motion` instead of a thumbnail.
- Changing wallpaper = writing the id to `rikka.appearance/wallpaper`. The Backdrop feature turns that into `show`. Readers use `shell_wallpaper::file_of_setting`.
- Backdrop: one background surface per output (`SurfaceDesc::wallpaper()`: `Buffering::Double`, drawn 1:1 so its texture never grows mips). `rikka_media::kind` picks picture or movie. Still pixels are decoded no larger than needed to cover the output (physical size from its `Placement`), decoded again when `on_moved` reports a new size for that output, and weakly shared by path, mtime, size and that cover size.
- Mask transitions hold until video opens. Animated images show one frame.
- A worker derives each wallpaper's Material seed into `rikka.appearance/material-source`.
- The background surface is only the wallpaper, because window captures show Background layers and nothing else. `Backdrop::beside_taskbar()` (Full) follows the taskbar's reserve; `Backdrop::alone()` does not.
- `Backdrop::shown_on()` hands out `ShownOn`, an observed per-output-name `Decision` (`Pending` from each wallpaper surface's start until its first wallpaper begins crossing, then `Made(id)`); the desk surface reads it so its widgets morph with the crossing.
