---
title: Task Manager application
description: rikka-sysmon probes plus the private tasks-process crate (rows, grouping, kill) under one virtualized window.
about: apps/system/task-manager
tags: [application, linux]
---

- CPU, memory, disk, interface and GPU probes come from `rikka-sysmon` (`crates/status`); the private `tasks-process` covers rows, grouping and kill.
- Each probe owns its previous counters. One worker samples each second, drops overlapping ticks, and discards the first delta. Probes are built in the first sampling task.
- Process memory is the written footprint (`Pss_Dirty + SwapPss` from `smaps_rollup`, `Private_Dirty` on kernels before 6.0), as a private working set: shared libraries and clean file pages are not counted, and written shared pages are split among their sharers. It shows as unavailable when inaccessible, never falling back to RSS.
- Expensive readings refresh every 4 s: the memory footprint and each process's identity (`cmdline`, program, uid, both validated by start time) and the DRM fdinfo sweep. In between, only `stat`, readable `io` and known GPU users are read. `Group` carries its `Totals`, computed once when grouped.
- Ending a task (`tasks_process::terminate(&Process)` for SIGTERM, `kill` for SIGKILL) opens a pidfd, checks the sampled start time and signals through the pidfd, so a recycled pid is never hit.
- GPU sources, in order: runtime NVML, amdgpu sysfs, DRM fdinfo (skipped if all adapters are NVIDIA). `pci.ids` loads once.
- NVIDIA usage (`rikka_sysmon::gpu`, `gpu/nvidia/`): adapter, encoder and decoder busy are the mean of NVML's buffered samples since the last reading (`nvidia/interval.rs`), not `utilization_rates`, whose ~200 ms window spikes. Per-process shares come from the `rikka-gpu-users` thread (started by the first `Gpu::sample`, polls every 80 ms): the mean of each process's sparse samples, held for 2.5 s when none arrived.
- Rows are self-painted collection cells with heat backgrounds that emphasize low use. A separate header owns sort and widths. Cell text is formatted once per row (per sample) on first draw and cached in the row. Rows live in `processes::RowTable`, an `Observed` cell holding a `Snapshot` the lines paint from: a sample repaints them without a view; `Manager`, `Processes` and `Performance` are tracked states and each tab body is a component, so only a changed row order (`RowOrder`: row names plus whether to glide), selection position or tab re-views.
- Rows are named (`Row::name`: the pid, or `1 << 32 | category` for sections). Only an expand/collapse rebuild glides (`RowOrder.glides`); samples, sorting and search settle at once. A line paints the live row at its index while the name matches the view's `RowOrder.snapshot`, else the snapshot, so collapsing ghosts keep their old rows. `RowOrder.order` is in every line's memo revision: name-keyed riders ignore index, so a changed order must rebuild every visible line.
- Performance: `Resource` is only the key a chart and card are stored under. `performance/monitor/` has one module per resource (cpu, memory, disk, network, gpu) implementing `Monitor` (colour, title, card summary, card scale, pane) over `MonitorData` (the reading and its series); `Performance::monitor` is the one match that builds them. The CPU's overall/logical choice is the nested `CpuView` state.
- Grouping uses parentage, session managers, user boundaries, repeated executables and desktop entries. The only action is terminate.
- Gaps: no per-process network. Some disk and GPU values are permission-limited and show 0. No root-only DMI memory or nl80211 SSID.
