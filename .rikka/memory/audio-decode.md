---
title: Sound decoding (rikka-audio-decode)
description: rikka_audio_decode::read door, gapless FFmpeg decoding, DSD reading, growing downloads, the pull Decoder and WavWriter.
about: crates/media/rikka-audio-decode
tags: [media, audio]
---

- Public `rikka-audio-decode` (needs rikka-ffmpeg and rikka-resample) owns decoding, DSD reading, `Sound`, `WavWriter` and its own `Error`. Format, music, Cut, Animate, Files and the hifi crates use it; rikka-audio uses it through its public API (`Decoder`, `Sound::samples`) and wraps its errors as `Error::Decode`.
- `read(src)` offers `.format() .decode() .decoder() .probe() .dsd()`. `AudioInfo` has `bits_per_sample` (lossless and DSD only), `frames` (exact trimmed length) and `encoding: Encoding { codec, lossless, bitrate, dsd_rate }`.
- `AudioFormat` is only a hint naming the stream for FFmpeg's probe; content is sniffed. The media format table is rikka-media-format, not `AudioFormat::from_extension`.
- Every codec decodes through FFmpeg (`decoding/ffmpeg`, `Error::NeedsFfmpeg(codec)` without it); `sniff.rs` only reads magic for the codec name and WavPack's hybrid flag.
- Output is sample-exact gapless: FFmpeg skips the priming samples, and when the length is exact (a whole-frame time base, iTunSMPB, Ogg Opus minus pre-skip) the backend clears FFmpeg's end discard and the pipeline's `Window` cuts the tail, because FFmpeg stamps Ogg Vorbis one packet late.
- Lossy seeks pre-roll 4096 frames; a seek FFmpeg refuses (tiny files) reopens the source and decodes forward.
- `Reading::dsd()` gives a `DsdReader` (raw 1-bit stream, byte-interleaved, MSB first); DST-compressed DFF is `NotDsd`.
- Any `StreamReader` (Read + Seek + `complete_len`, also boxed) decodes with `reader_decoder`, whose `info()` is the probe; its reads may block on bytes still arriving. `complete_len` must stay `None` until every byte is readable, or the probe reads the tail and waits.
- A source of unknown length opens without a seek callback and with one probe packet (`rikka_ffmpeg` `InputContext::open_source`), so opening reads only the head (about 100 KiB for WAV, a few frames otherwise); seeking reopens it once the length is known.
- `read(src).decoder()` is a pull `Decoder` (`seek` sample-accurate, `read` appends interleaved f32) for offline sound.
- `WavWriter::create(path, rate, channels, WavDepth::{Pcm16, Pcm24})` writes RIFF WAV (1–18 channels, extensible above 16-bit or 2 channels, TPDF dither at 16-bit, `WavTooLarge` past 4 GiB).
