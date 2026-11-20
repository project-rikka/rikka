---
title: System monitor (rikka-sysmon)
description: Shared CPU/memory/disk/network/GPU probes, hwmon sensors, RAPL, volumes, and the on-demand Machine sampler.
about: crates/status/rikka-sysmon
tags: [sysmon, metrics]
---

- `crates/status/rikka-sysmon` is a public crate; GPU probing (NVML, amdgpu sysfs, fdinfo) is its `rikka_sysmon::gpu` module (`Gpu`, `GpuSample`, `Adapter`, `Memory`). Task Manager uses the raw `Probe`/`Gpu`.
- `Machine::start()` (1 s) or `Machine::with_interval(d)` runs a `rikka-sysmon` worker that samples only while some `changes()` stream is alive; idle it blocks. After waking it discards one reading and waits 250 ms so the first delta is meaningful.
- `Vitals` (shared as `Arc`) holds CPU, `cpu_power` (RAPL packages, usually root-only so `None`), load, memory, GPUs (with NVML/hwmon `power`), disks, `volumes` (mountinfo real filesystems, one per device, shortest mount), networks and `Sensors`.
- Mounts come from `rikka-mounts` (`crates/desktop`), the workspace's one `/proc/self/mountinfo` reader; `rikka-sysmon` only turns them into `volumes`.
- Helpers: `cpu_temperature()` (k10temp/zenpower Tdie→Tctl, coretemp package, ARM thermal, label "CPU", acpitz), `main_gpu()` (most dedicated memory), `traffic()` (up Ethernet/Wi-Fi only), `io()`, `system_volume()`.
- The sampler uses `Gpu::adapters()` (no per-process collection; fdinfo only when a card has neither NVML nor amdgpu sysfs) and remeasures volumes every 30 s.
- hwmon inputs are rediscovered every 60 s; zero or implausible temperatures are dropped.
