---
title: Packet playout jitter buffer (rikka-playout)
description: Public adaptive jitter buffer and queue target shared by Discord voice and the remote viewer.
about: crates/media/rikka-playout
tags: [media, audio]
---

- `Playout<D: Decode>::new(decoder, origin)`, `push(Packet { timestamp, arrived, payload })`, `pull(now, &mut [f32; 960])` → `SpeechType::{Silence, Concealed, Speech}` (ordered) per 10 ms stereo chunk; `target()`/`buffered()`.
- `Decode` (frames, decode, recover-from-next-packet FEC, conceal) is implemented for `rikka_opus::OpusDecoder` in `src/opus.rs`; tests use a sine "codec" that counts decodes. It is fixed at 48 kHz stereo (`RATE`, `CHANNELS`, `CHUNK` = 10 ms).
- Users: discord-voice (one `Playout<OpusDecoder>` per SSRC) and rikka-remote-viewer (`session/sound.rs`, which pushes a packet's redundant previous frame as its own packet one frame earlier); both pace output with `QueueTarget` against `AudioFeed::wait_until_queued_below`.
- Packets are keyed by unwrapped RTP timestamp, so DTX gaps and talkspurts need no sequence logic; late packets still feed the delay estimate.
- An empty buffer expands with 10 ms of concealment without advancing the playout timestamp, so a late packet is still played when it lands; expansion stops after 120 ms or after an inaudible frame and the stream idles until the next talkspurt buffers `target`.
- A missing frame with later packets present is lost: FEC from the next packet when adjacent, else concealment, and the timestamp advances.
- `delay.rs`: relative delay against the fastest packet of the last 2 s, 10 ms buckets, forget 0.983 with a fast start, 95th percentile + packet + 10 ms, 30–500 ms.
- `stretch.rs`: one pitch period (2.5–10 ms, correlation ≥ 0.9, or 10 ms when quiet) is cut when the smoothed level exceeds max(target + 30 ms, 1.5·target) and repeated, at most every 200 ms, below target − 25 ms.
- `QueueTarget` sizes the speaker queue: `observe(now, queued, underruns)` learns the largest device read of the last 3 s (+15 ms, ≥ 30 ms), adds 10 ms per feed underrun (eased back after 20 calm seconds), capped at 180 ms; `wrote` counts what was queued.
- `tests/support/device.rs` simulates a PipeWire-like device (quantum, 4 ms receiver poll, QueueTarget) for quanta 256–4096 and jitter up to 120 ms.
