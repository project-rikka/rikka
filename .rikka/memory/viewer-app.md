---
title: Viewer application
description: Opens pictures, audio, video, 3D models and text with bounded probing, playback, figures and text editing.
about: apps/system/viewer
tags: [application, viewer]
---

- For transient viewing. Non-text editing and asset management belong elsewhere.
- `viewer-media` probes at most 8 KiB (signatures first, then extension, no decode), so `main` can pick the window and screen before slow work.
- It delegates media formats to `rikka-media` and keeps model and text formats. `viewer-text` owns encoding and line-ending policy.
- Images use a custom zoom and pan stage (`screen/canvas.rs`: pointer-anchored zoom, unbounded drag at any zoom, a drag in Fit becomes `Zoom::Scale`), with the built-in image widget as fallback. Animated images show one frame. Pictures over 2560 px are first decoded off-thread into a 2560 px `Proxy`; once the shown size outgrows it the full file is drawn as a `Sprite` with the proxy as `placeholder`, so it never blanks while the full picture (evicted when zoomed out) decodes.
- PSD/PSB are `Kind::Picture` formats of `viewer-media` (`Format::is_photoshop`, extent from `rikka_psd::FileHeader`). `viewer-psd::flatten` shows the stored composite when `has_real_composite`, else renders layers with rikka-psd-render; it refuses documents whose estimated working memory exceeds 2 GiB, checked from the header before reading. The pixels reach the canvas as `ImageSource::Rgba`.
- Video is a `rikka_movie::Movie`; its `on_change` marks an `Observers` the player's reads track, so the transport follows playback without a tick; music does the same through `Track::on_change` (only animated figures still tick).
- Models: `Kind::Model` covers every `rikka-model` importer. `screen/figure` keeps model, camera and pose in one `Scene` behind `Arc<Mutex<_>>`. The `Grip` overlay turns input into camera moves.
- `Scene::build` returns nothing when nothing changed. Dropped models go to the reclaimer.
- Animated models are `Player`s with a combo to pick an animation. A `.vmd` asks for a model and binds via `Vmd::animation`.
- Toon models are lit softly without shadows. Others get a key light with shadows plus a fill. The background stays transparent.
- Text: `TextDocument`, Ctrl+S (atomic rename, keeps encoding, endings and a UTF-8 BOM, reports UTF-8 fallback), Markdown rendered/source toggle, `Ctrl+F`/`Ctrl+H`. Text cut at 32 MiB opens read-only and never saves. Folder stepping is blocked while text is unsaved. Links open relative files and `file:` URIs (percent-decoded).
- Stepping and links probe the next file off-thread into an `OpenFile` (`Viewer::visit`, the newest navigation wins); music and movie sound tracks open off-thread (`screen/sound.rs`), and a track that fails to open shows its error. Players share one `Ticker` (`screen/player.rs`). A model is read and prepared for the GPU (`rikka_model_gpu::Prepared`) on the loader thread (`figure/load.rs`, `Purpose::Motion` reads a motion file alone); the scene only uploads it on the render thread and signals the screen through a oneshot once the upload finished, instead of a timed refresh.
- Installed `mimeapps.list` defaults Viewer (user config still wins). `rikka-viewer.xml` defines `model/vrm`, `model/x.fbx`, `model/x.pmx`, `model/x.vmd`, with `*.vrm` weight 60 over `model/vrml`; STL and `.blend` use the shared database's `model/stl` and its own database type.
- It claims every picture type rikka-image reads, including AVIF, HEIF, JPEG XL and JPEG 2000 (decoded through FFmpeg). No model cameras, lights, wireframe or picking.
