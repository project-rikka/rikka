---
title: Graphics driver selection and Rikka's own Mesa (rikka-gpu-drivers)
description: rikka-gpu-drivers picks drivers per render node: pinned Mesa for Rikka processes, narrowed system drivers for others.
about: crates/graphics/rikka-gpu-drivers
tags: [gpu, drivers]
---

- Rikka processes run on the pinned nixpkgs' glibc and libstdc++, so they cannot load a system Mesa built on a newer one. They therefore use the pinned Mesa shipped beside them (`lib/rikka-mesa`, found by `rikka-shipped`, `RIKKA_MESA_PATH` in dev runs and tests) for Mesa-driven GPUs; only NVIDIA's proprietary driver comes from the system.
- The kernel-driver table (`kernel.rs`) maps each render node's driver (`/sys/class/drm/renderD*`, sorted) to Mesa Vulkan drivers, vendor ICD manifest prefixes and EGL vendors. The shipped plan skips unknown nodes; `narrow_system` gives up on any unknown node.
- `Drivers::shipped()` (None without the shipped Mesa or a known node) gives `vulkan_libraries()`, which rikka-gpu's `VulkanInstance` loads itself and hands to the loader through `VK_LUNARG_direct_driver_loading` in EXCLUSIVE mode (`core/direct_drivers.rs`). That ignores system ICDs and `VK_LOADER_DRIVERS_SELECT`, and touches no environment, so nothing leaks to child processes. `VK_DRIVER_FILES`/`VK_ICD_FILENAMES`/`VK_ADD_DRIVER_FILES` turn it off.
- On the direct path the instance never calls `vkEnumerateInstanceExtensionProperties(None)`: that query makes the loader dlopen every system ICD. It assumes the loader's own extensions instead (loader ≥ 1.3.236).
- `Drivers::environment()` is for children Rikka controls that use the system loaders: rikka-web applies it to the Gecko process (sets `VK_DRIVER_FILES`, `__EGL_VENDOR_LIBRARY_FILENAMES`, `GBM_BACKENDS_PATH`, `LIBVA_DRIVERS_PATH`; removes the session's `VK_LOADER_DRIVERS_SELECT`).
- Rikka's own GBM goes through `runtime.nix`'s libgbm, built to search the pinned Mesa first, in the dev shell and the distribution alike.
- `narrow_system()` is the compositor's start-up narrowing of system drivers for every session client (`VK_LOADER_DRIVERS_SELECT`, `__EGL_VENDOR_LIBRARY_FILENAMES` when one kernel driver), exported through the session environment.
