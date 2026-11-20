---
title: Desktop portal ownership
description: Files owns FileChooser, rikka-cast owns ScreenCast; each picker or dialog is its own helper process.
about: system/rikka-cast
tags: [portal, system]
---

- `rikka-portals.conf` assigns each interface separately.
- `apps/system/files` (`files-portal`) implements FileChooser. `system/rikka-cast` (`rikka-portal-cast`, `--picker` for the source picker) implements ScreenCast.
- Backends are D-Bus-activated and hold live PipeWire work. Pickers and dialogs (`rikka-file-dialog`) are separate processes, so closing the UI never ends a cast.
- `rikka-portal` (serde only) is the JSON pipe protocol helpers link: `Outcome`, `Reply`, `read`/`write`. `rikka-portal-backend` (zbus) is what backends link: `Helper` runs a helper per request and answers `Request.Close`, `serve` claims the bus name, plus the response codes. Neither owns UI.
- A helper reads its ask from stdin and answers only through `rikka_portal::Reply`: `Reply::claim()`, called first in the helper, keeps the original stdout as a close-on-exec answer pipe and points fd 1 at stderr, so logs, libraries and children never corrupt or hold the answer. `rikka-app` logs to stderr.
- Portal config and service files must be installed. Running bare binaries gives an inconsistent desktop. The compositor restarts stale portals after publishing its environment.
