---
title: Shell capture and clipboard history
description: rikka.capture's frozen-frame picker, screenshots, recordings and snips; memory-only clipboard history.
about: system/rikka-shell/crates/shell-capture
tags: [shell, capture]
---

- `rikka.capture` is one picker, selection model, shortcut family and service. `Mode` decides the result: a kept picture, a recording, a translate snip, or a code scan.
- `Mode::Scan` (`Super+Shift+Q`, `Shortcut::ScanCode`) freezes without the cursor and keeps the picker up. `src/scan/` owns it: `scanner` reads every panel (and dragged areas through `read_framed`) on threads, `ScanResults` hand results back via `rikka.capture/scanned`, `Codes` (in `Shared`) holds codes, chosen, a screen scan and one turn-numbered area scan (stale turns are dropped), and missed. `point`/`act` methods choose a code and act; `Act::primary(&Meaning)` is the one rule for the main action.
- The card hangs below the chosen code inside the marquee (`scan::beside`); the marquee undims each code with a clipped redraw and outlines it, takes Tab/arrows to cycle, Enter for the primary act, Ctrl+C to copy. Nothing scanned is kept after the picker closes; copied Wi-Fi passwords use `Offer::secret()`, which clipboard history skips through `Clipboard::is_secret`.
- `shell_kit::launch::Spawn::request()` is the one way to build a `rikka.launch/spawn` request.
- Link and Wi-Fi interpretation lives in the shell-private crate `code-meaning` (only http/https are links, with cautions); joining is `Network::accept` and a follower thread that notifies the outcome.
- Every output is frozen before selection, so the pixels match the selection.
- The picker draws the frozen frame under one dim quad with a `cut_out` for the chosen area and an accent outline between two dark hairlines; the pointer is only the system crosshair cursor.
- Both picker surfaces are animated on `picking` with `shell_kit::EXIT_DURATION`: the sheet (`modal`) fades in and out as one opacity group and holds its last area while leaving (`MarqueeState::held`); the toolbar sits flush on the top edge (top margin as pointer-inset padding), slides its glass bar past it and closes its target list on leave. A released drag is `Selection::settle`d, not cleared, until the service closes the picker.
- Closing the picker starts `Deferrals` (`src/deferral.rs`): picture, video and pick requests and the chosen recording (`Deferred::Record`) wait until `Presence::after_unmap` calls `rikka.capture/closed`, so nothing freezes or records the leaving picker. A held pick or `Super+R` disarms a held recording.
- Selection is in global logical coordinates. Screenshots stitch at the highest touched scale. Recordings take the dominant output, and window recordings follow the window.
- Encoding, stitching, writes and clipboard publication run off the loop. Results notify through the shell's notification service.
- Screenshot cards carry the markup errand; recording cards carry the trim errand.
- `Super+R` toggles recording. Quality is 1–100 constant quality. `Capture::recording()` hands the taskbar a `shell_feeds::ScreenRecording` (since when it runs, and the `rikka.capture/stop` request its red stop button sends); a recording that ends on its own calls `rikka.capture/settle` from the recorder thread (`rikka_record::Recording::start`'s `ended` callback).
- Frames are placed by capture time, so still screens leave gaps instead of repeats.
- Clipboard history is session memory only. Password-manager secret offers are skipped. It keeps text, file URI lists or one image.
- A recorder thread does transfer, decode and thumbnails, reading every clipping with `Clipboard::read_within` so nothing over the per-clipping limit is buffered. Its own publications are not re-recorded.
- The byte budget covers everything including pins. A clipping pins leave no room for is refused, not evicted. Duplicates move to newest.
- The search key is lowercased once and counted in the budget.
- The picker is `shell_kit::popover` + `finder` + `nothing` (shared with emoji), placed by `SurfaceDesc::aimed` like emoji. Auto-paste types `Ctrl+V` after the overlay is gone.
- Both need privileged `ext-data-control-v1`; without it, fail explicitly.
