---
title: Updates application (rikka-updates)
description: Single-instance viewer of rikka-nixctl runs: run list, header and progress, a paged then live log.
about: apps/system/updates
tags: [application, nix]
---

- `rikka-updates [--run <id>]` is single-instance; a second launch selects the run in the running window (`Window::on_open` → `Updates::opened_with`). The shell's update notification starts it.
- `Updates` holds one `rikka_nix::Monitor` (`Interest::Transcript`), the runs newest first, the selection and a `Log` child state. `Monitor::watch` and `runs()` run in `spawn_blocking`.
- `Log` pages the transcript in with `Monitor::transcript(run, from)`, then appends live `Lines` through `rikka_nix::Tail` (a `Gap` refetches) into a read-only `TextDocument`, following the end unless scrolled away. Find, copy and save work on that document.
- `Event::Exited` marks running runs interrupted; `Event::Disconnected` also drops the monitor and shows the retry page, so retry opens a new watch.
- `describe.rs` owns the wording of operations, phases, tallies and failures; the wire and run types are described in the System updates note.
