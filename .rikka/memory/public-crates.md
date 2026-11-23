---
title: Public crates used by apps
description: //visibility:public libraries by layer, the narrower exceptions, rikka_app::prelude, errors and shared helpers.
about: crates
tags: [api, layering]
---

- Apps, system binaries and app-private crates depend directly on public crates; there is no facade.
- `visibility = ["//visibility:public"]` on a `rikka_library` makes it public; every other `crates/` library is visible only to `//crates`, `//benchmarks` and `//xtask` subpackages, and Bazel enforces it.
- A crate apps newly need gets `//visibility:public`; its public signatures name only public crates' types. Public crates' docs build with warnings denied: `bazel build //<crate>:<name>_doc`.
- Public (non-test, names without `rikka-`): foundation `archive, color-formats, color-lookup, dirs, emoji, encoding, file-edit, file-format, format, fs, fuzzy, heap, i18n, input, interfaces, keyframe, languages, log, net, oauth, pipewire, public-suffix, simd, sync, types, watch, wayland, wayland-client, wayland-protocols, worker, xml`.
- desktop `account-book, clipboard, clipboard-core, dialog, fido, file-operations, mime, mounts, notify, open, pam, screencast, secrets, thumbnails, trash, xcursor, xdg`; session `appearance, blur, capture, portal, portal-backend, present, record, seam, session, settings, virtual-input, window-view`; status `nvml, sysmon, weather`; win32 `swapchain` (Windows-only); daemons: every crate except `keyring-vault` and `test-objects`.
- graphics `barcode, colorimetry, effects, figure, gpu, gpu-drivers, icons, ies, image, image-edit, path-data, picture, psd, psd-render, renderer, renderer-3d, shader-compile, shader-graph, shader-presets, shader-presets-gpu, slang, symbols, text, theme, theme-seed`; text `text-unicode` only.
- media `apm, audio, audio-decode, coded, hifi, media, media-format, movie, opus, playout, resample, video, video-play, vulkan-video`; model `humanoid, mesh-bevel, mesh-boolean, mesh-edit, mesh-modifiers, mesh-reduce, mmd, model, model-core, model-gpu, polymesh, spatial, vrm`; physics `physics, physics-liquid, physics-tailor`; nix `nix, nix-journal, nix-log, nix-protocol`; remote `remote-link, remote-wire`.
- ui `animation, app, effects-forms, figure-editing, markdown, seat, syntax, terminal, timeline, ui, ui-host, ui-video, viewport, widgets`; web `html, omnibox, web`.
- ai `agent, assist, chat, computer, computer-use, llm, llm-accounts, llm-providers, mcp, prompt, thread, tool, tool-docs, tools, usage-bar`; proton `proton-{account,api,authenticator,cache,client,contacts,drive,key-transparency,mail,names,pass,pgp}`.
- Narrower grants: `annexb` and `rate-control` are also visible to `//apps/system/discord` (rate-control also to `//system/rikka-remote`); every compositor crate, `wayland-wire`, `wayland-scanner` and `nested` to `//system/rikka-compositor`; `nix-edit` and `nixos-layout` to `//installer`.
- Test helpers are `testonly`; all are public except `test-display` (compositor) and `test-objects` (daemons only).
- `rikka_app::prelude::*` is the usual UI import: `App`, `Window`, `LaunchRequest`, `Instance`, `Sender`, `open_window`, the rikka-ui view, layout, motion, task and text vocabulary, every `rikka-widgets` item, `rikka_theme::{Corner, Palette, Theme}`, `tr!`/`args!` and core `rikka-types`. Markdown and `rikka_syntax::Language` are imported explicitly.
- `#[derive(State)]` and `#[derive(Widget)]` expand to `::rikka_ui` paths, so users depend on `rikka-ui`; only rikka-ui itself passes `crate = "crate"`.
- Every crate keeps its own `Error`/`Result`; app mains return `rikka_app::Result<()>`.
- The process-wide picture engine on the UI device is `rikka_image_edit::Imaging::shared(&compute)`, with the compute from `rikka_picture::SharedGpu::compute(timeout)` or `SharedGpu::on_compute(callback)`. Bundled UI languages are `rikka_i18n::languages()`.
- SVG path data is read once by `rikka_path_data::segments` (absolute segments, arcs as cubics, byte-offset errors). A pan/zoom view over content of known extent is `rikka_widgets::PanZoom`; an accent-tinted tool button is `rikka_widgets::tool_tile`.
- Wayland clipboard data lives in `rikka-clipboard`; the process clipboard calls (`text`, `set_text`, `read`, `publish`, `watch`, `files`) are `rikka_ui::clipboard`.
- Live settings handles are `rikka_settings::seam::{Settings, Preferences, Changed}`; `WindowView` is in `rikka-types`, `WindowViews` in `rikka-window-view`.
