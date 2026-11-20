---
title: Video playback (rikka-video-core, rikka-video-play, rikka-movie)
description: The video crate split, VideoPlayer opening and pacing, the reaper, VideoController, Movie with its soundtrack.
about: crates/media/rikka-video-play
tags: [media, video]
---

- Video is three crates: internal `rikka-video-core` (demux, decode, swscale conversion `SwConverter`, Vulkan interop, FFmpeg Vulkan layouts, `VideoInfo`/`probe`, the reaper), public `rikka-video-play` (`VideoPlayer`, `Opening`, `Ready`, `PlaybackState`, `DecodeBackend`, `wait_for_closed_players`) and public `rikka-video` (stills, reading, coding; video-coding note).
- `rikka_picture::VideoController` (`open`, `play`, `pause`, `seek`, `set_looping`, `set_speed` 0.01–64×, `state`, `position`, `changes()` yielding on size or state changes, `watch(|VideoEvent|) -> VideoWatch` calling back on the host thread with `Changed` and `Progressed`, so nothing polls) is the UI-facing, picture-only handle; `rikka_ui_video::VideoStore` plays controllers through `VideoPlayer` for the UI host's `VideoPlayback` seam.
- `VideoPlayer::opening(device, path, backend, wake)` probes and starts off-thread, then `Opening::ready` → `Ready::play_on(texture)`; `VideoPlayer::open(&Uploads, path)` is the blocking form. Frames are zero-copy on the renderer's device; playback needs a `Gpu` with `video_decode()`.
- Decoder and player share a `Mailbox` (3 events, controls); the decoder blocks when it is full or after the end, and `wake` fires when the open answers and when an event lands on an empty queue, so a playing video wakes its host only when a frame is due (`next_due`) or arrives.
- A paused player still shows its frame after a seek. Closing never waits: open, decoder and `FrameReader` threads are `rikka_worker::Worker`s spawned through `rikka_video_core::reaper::spawn(name, stop, body)`, whose `Reaped` handle stops them and hands the join to the reaper thread on drop; `wait_for_closed_players()` waits once at exit.
- Decoder output surfaces (`decode.rs` `get_format_vulkan`) set `img_flags = interop::SURFACE_FLAGS` (MUTABLE_FORMAT | EXTENDED_USAGE) without ALIAS, which no H.264 profile lists. Plane views that only sample (rikka-gpu `video/imports.rs`) or only store (`compute/foreign.rs`) narrow their usage with `VkImageViewUsageCreateInfo`.
- `probe(path)` demuxes only; `VideoInfo.frame_rate` is the nominal `r_frame_rate` (1–240 fps), else the average.
- Video is picture-only; video with sound is public `rikka-movie` over rikka-picture, rikka-audio and rikka-audio-decode. `Movie::open(path)` owns a `VideoController` and its own streamed soundtrack (opened on `rikka-movie-sound`, reopened when a finished stream is played or sought), forwards play/pause/seek/volume/looping and reports `position`, `duration`, `picture()`, `soundtrack()` and one `on_change(|MovieEvent|)` listener (`Changed`, `Progressed`, `Ended`).
- A movie is driven only by the controller's `watch` events: a picture within 400 ms settles a seek, sound more than 600 ms from a playing picture is re-sought, and an ended or failed picture stops it. Viewer, Discord, the SDK video and the shell's trim preview use it.
