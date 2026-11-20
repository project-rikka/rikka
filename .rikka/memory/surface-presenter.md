---
title: Wayland surface presenter (rikka-present)
description: rikka-present draws a wl_surface into its own exported dmabufs with explicit sync, buckets and a viewport; no WSI.
about: crates/session/rikka-present
tags: [wayland, presentation]
---

- `Presenter::new(&Host, &WlSurface, &mut Renderer, SurfaceConfig)` makes one per surface; it implements `rikka_renderer::Present`, so `Embedder<K, S, Presenter>` drives it. rikka-app windows, its drag icon and every shell layer surface use it; it is the only way a client presents.
- API: `resize((w, h) px, scale, interactive)`, `set_transparent`, `set_transform`; `Present::present` offers the buffer (attach, `damage_buffer`, syncobj points), sets the viewport, then `wl_surface.commit` and a flush. A `Recorded` that does not go up commits nothing.
- It owns the surface's `wp_viewport` (source = visible px, destination = px / scale) and sets `buffer_scale` 1, so hosts must not set either. Without a viewporter it falls back to an integer `buffer_scale` and exact buffers.
- Own buffers (`pool.rs`) need zwp_linux_dmabuf_v1 ≥ 4 with surface feedback and `Gpu::dmabuf_export_formats` (`renderer.gpu()`); the first feedback tranche (scan-out ones first) whose formats the GPU exports picks fourcc (XR24 opaque, AR24 transparent) and modifiers. `Presenter::new` does not wait for the feedback: `acquire_image` returns `NotReady` until it arrives and `Error::NoLayout` when no tranche fits; `Error::NoExport` or a missing global fail `new`. There is no swapchain fallback (the renderer has no WSI path).
- Buffers: `Renderer::export_dmabuf` + `create_immed`; at most 2 (`Buffering::Double`) or 3; drawn with `DmabufOutput::with_target(..).visible(w, h)` so only the visible part renders and per-buffer damage history keeps working. Interactive resizes allocate 128 px buckets (`rikka_renderer::Sizing`, shared with rikka-dcomp) and reuse any free buffer within two buckets; a non-interactive size wants exact buffers, and a shown buffer that no longer fits damages the whole scene so the next frame refits.
- Linux-only (`target_compatible_with`), like rikka-blur; Windows presents through rikka-swapchain.
- Sync: with wp_linux_drm_syncobj_v1, each buffer has its own DRM timeline (render node opened from `Gpu::drm_nodes`): the frame's sync file is imported into a scratch syncobj and transferred to the acquire point; reuse waits for the release point (signalled → free; only materialized → its sync file becomes the canvas wait via `DmabufDraw::with_release_fence`). Without it: wl_buffer.release plus DMA_BUF_IOCTL_IMPORT/EXPORT_SYNC_FILE, falling back to a CPU wait on the paint thread.
- `acquire_image` waits on the paint thread only (timeline wait or the queue's waker); events arrive through the presenter's own queue (`EventLoop::poll_dispatch`), filled by whichever thread reads the connection.
- `trim()` (the embedder calls it `TRIM_AFTER` after the last shown frame) drops every free buffer, keeping only the one on screen.
- Measured with the compositor's `frame_cost` bench `own-*` scenarios; tested only there (compositor-side) and by the renderer's export test.
