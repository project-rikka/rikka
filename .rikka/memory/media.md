---
title: Media crates map and shared conventions
description: crates/media layout, the read(source) door convention, rikka-ffmpeg, rikka-media(-format), coded, opus, resample.
about: crates/media
tags: [media, api]
---

- Every media door takes `impl Into<Source>` (path, str, bytes, `Arc<[u8]>`); background jobs store `into_owned()`.
- Pictures: rikka-image and rikka-image-ffmpeg (image-codecs note). Sound: rikka-audio-decode (audio-decode note), rikka-audio (audio-engine note), rikka-hifi (hifi-output note), rikka-playout (playout note), rikka-apm (apm note). Video: rikka-video-core, rikka-video-play, rikka-movie (video-playback note), rikka-video (video-coding note), rikka-vulkan-video (vulkan-video-encoder note).
- rikka-ui reaches none of the video crates nor rikka-audio; programs that play video install `rikka_ui_video::VideoStore` (ui-crate-layering note).
- FFmpeg is dlopened by the internal `rikka-ffmpeg` (`load()`, `load_scaling()` for swscale; hand-bound layouts, owned wrappers `CodecContext`, `InputContext`, `OutputContext`, `BufferRef`, `Sws`, custom AVIO over readers; log level from `RIKKA_FFMPEG_LOG`). Its users are the video crates, rikka-audio-decode and rikka-image-ffmpeg; each returns `Unavailable`/`NeedsFfmpeg` without it.
- On Windows rikka-ffmpeg loads `avutil-61.dll`, `avcodec-63.dll`, `avformat-63.dll`, `swscale-10.dll` from the executable's folder or System32 only (`LOAD_LIBRARY_SEARCH_APPLICATION_DIR | SYSTEM32`), and paths must be Unicode (FFmpeg takes UTF-8). The bound structs hold no C `long`, so the layout assertions hold on LLP64 too.
- Linux-only media targets: rikka-pipewire, rikka-hifi, rikka-dmabuf-convert, rikka-vulkan-video; rikka-video cfgs its dmabuf path (`Feed::Dmabuf`, `import`, `encode_imported`, `push_imported_at`, `Previewer`, `Preview`, `Imported`) to unix and skips VA-API in the Windows hardware order.
- libopus is opusic-sys `bundled` (CMake, static) on every platform.
- `bazel run //xtask -- ffmpeg-layout` regenerates the layout assertions of rikka-ffmpeg and rikka-video-core from `manifests/` (needs FFmpeg dev headers on `PKG_CONFIG_PATH`, Vulkan headers on `VULKAN_SDK`, `pkg-config` on PATH and no kache `clang` shim on PATH).
- Audio and picture test fixtures are committed files; hermetic remote tests load FFmpeg through the session RUNPATH.
- `rikka-media-format` (public; needs only rikka-image and rikka-audio-decode's `AudioFormat`) is the one media format table: `identify`, `kind`, `Format`, `Detail`, `Kind`, `Extent`. `Format::from_path` goes by extension, `Format::identify(path, header)` by bytes first; `detail()` is `Detail::Picture(format) | Sound(Option<AudioFormat>) | Movie` and `kind()` derives from it.
- `rikka-media` adds the decoding doors on top: `still`, `still_with_max_size(path, w, h)` (fits pictures, asks rikka-video's poster for a bounded movie frame, never enlarges) and `info`. `viewer-media` keeps only model and text formats.
- `rikka-coded` (public, dependency-free) holds the packet types `Coded` and `Sample`; `rikka-annexb` is the Annex B filler-NAL removal shared by rikka-video, rikka-vulkan-video and discord-rtp.
- `rikka-opus` (public, only libopus): `OpusEncoder::new(OpusApplication::{Voip, Audio, LowDelay}, channels)` at 48 kHz (`set_signal`, `set_bitrate`, `set_in_band_fec`, `set_expected_loss`, `encode`) and `OpusDecoder::new(channels)` (`decode`, `recover`, `conceal`); used by rikka-audio's sound sender, rikka-playout, discord-voice, rikka-remote and the remote viewer.
- `rikka-resample` (public, only rikka-simd): `Resampler::with_quality(quality, from, to, channels)` streams, `resample(samples, channels, from, to, quality)` converts a buffer; used by rikka-audio, rikka-audio-decode, rikka-video, rikka-hifi-core, cut-export and discord-voice.
- The resampler keeps its window planar per channel (`Planes`) and renders whole blocks through `Kernel::render`. The sinc kernel builds a cubic-Lagrange table (`SincTable`) and, when the reduced ratio's phase count fits, one exact weight row per phase (`PhaseBank`); otherwise `Interpolator` blends four rows per frame. `convolution::convolve` is the one rikka-simd dot kernel, dispatched per block on the level detected at construction.
- `rikka-ring` (internal) is the one wait-free single-producer sample ring (`channel`, `Producer`, `Consumer`, `Reservation`, `pop_slice_before`, `skip_to`) under rikka-audio and rikka-hifi. `rikka-fft` is the real FFT shared by rikka-apm-agc2 and rikka-sound. `rikka-rate-control` is the sans-IO pacer and congestion controller (remote-protocol-transport note).
