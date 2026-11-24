---
title: Windows Vulkan swapchain presenter (rikka-swapchain)
description: rikka-swapchain presents rikka-renderer frames into an HWND through a VK_KHR_win32_surface FIFO swapchain.
about: crates/win32/rikka-swapchain
tags: [windows, presentation]
---

- Windows-only public crate `//crates/win32/rikka-swapchain` (`rikka_swapchain`), the Windows counterpart of rikka-present; it implements `rikka_renderer::Present` for `rikka_ui_host::Embedder`. Windows are opaque: no Mica, no alpha.
- API: `Presenter::new(window: isize, &mut Renderer, SurfaceConfig)` (only `buffering` matters; swapchains are always opaque), `resize((w, h))`, `take_presented() -> Option<Presented { at, refresh }>` (first DWM vblank after the present, `DwmGetCompositionTimingInfo`).
- Vulkan pieces live in rikka-gpu `win32/`: `Surface` (`fit(requested) -> Option<SurfaceFit { extent, readable }>`, `None` when minimized), `Swapchain` (FIFO, `clipped(false)` so images keep contents, B8G8R8A8/R8G8B8A8 UNORM, SAMPLED+TRANSFER_SRC when offered, per-image acquired/rendered binary semaphores with the spare-semaphore swap, `present` under the graphics queue lock with VK_KHR_incremental_present rects, `Drop` waits the queue idle). `Win32Device` exists only when the instance has VK_KHR_surface + win32_surface and the graphics family presents; presenting devices get +10000 selection score.
- rikka-renderer `SwapchainOutput`: one target lease per presenter; `start_swapchain(scene, &output, fit)` plans; `swapchain_draw(&mut output, &mut swapchain, index)` binds a `BufferId` per image of the current swapchain (`Swapchain::id`), so new images draw whole; drawing yields `SwapchainFrame::{Submitted, Untouched}`, and only untouched images are held for the next frame.
- `acquire_image` queries the surface only when the swapchain is missing, out of date or differs from the frame `start` planned.
- Threads: `start`/`record_image` run on the paint thread under the renderer lock and never call WSI functions that may `SendMessage`; `acquire_image` (paint thread, no lock) re-fits/recreates the swapchain and acquires; `present` runs on the UI thread. Keep the message pump free of waits on the paint thread.
- `acquire_image` returns `Stale` when the surface fit changed since `start` or the swapchain is out of date; an acquired image the renderer did not draw is held for the next frame.
- Wine (winevulkan) runs it, so the general apps can be tried on Linux: Xvfb, lavapipe (`VK_DRIVER_FILES` + `VK_LOADER_DRIVERS_SELECT=*lvp*`), `HKCU\Software\Wine\X11 Driver\Decorated=N`.
