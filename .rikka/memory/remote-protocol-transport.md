---
title: Remote desktop wire format and transport (rikka-remote-wire, rikka-remote-link)
description: rikka-remote-wire's messages/datagrams and PROTOCOL.md; rikka-remote-link's QUIC, FEC, congestion control, pacer.
about: crates/remote
tags: [remote, network]
---

- `crates/remote/` holds two public crates shared by `system/rikka-remote` and `apps/general/remote`; a future client on another OS implements `rikka-remote-wire/PROTOCOL.md`, which is the normative spec (incl. the scan-code → evdev table).
- `rikka-remote-wire` is dependency-light encode/decode only: `ClientMessage`/`ServerMessage` (control stream, `Header` + body, at most `LONGEST_MESSAGE`), `InputEvent` (input stream, first byte `StreamKind::Input`), `FileHeader` (file streams), `ServerDatagram`/`ClientDatagram` (`VideoShard`, `AudioPacket`, Padding, Pointer, `Motion`, `Feedback`). Constants: `ALPN`, `VERSION`, `DEFAULT_PORT`, `POSITION_EXTENT`, `AUDIO_*`, `CURSOR_SHAPES`.
- `rikka-remote-link` is policy over the wire format, sans media: `fec` (Reed–Solomon Cauchy over GF(2⁸), SIMD nibble kernel), `Packetizer` (frame → shards + parity), `Assembler` (shards → frames, reorder grace, silence expiry), `Chain` (which frames are decodable given references), `FeedbackBuilder` (client arrival reports) and `acknowledgement(&Feedback)`, which hands a wire feedback to the congestion controller.
- Pacing and congestion control live in `crates/media/rikka-rate-control` (sans-IO, shared with discord-rtc): `Pacer<T: Paced + Clone>` (5 ms burst budget, urgent lane ahead of frame-tagged video, `withdraw(frame)`, probe clusters cloned from one padding packet, `pop(now)` → `Outgoing { packet, probe }`, `wake_at(now)` for deadline sleeps), `Congestion` (GCC-style trendline delay, AIMD, loss-based, probing; fed `Sent` and `Acknowledgement { base_seq, count, arrivals }`), `History`, and `LossBased` alone (RTCP-reported `report(fraction, now, rtt)`, `set_receiver_estimate` REMB clamp, `raise_to`).
- `quic/`: `server_endpoint`/`client_endpoint` (dual-stack, big buffers, DSCP), `client_config(&Trust)` with TOFU `Pin` verification (`Trust::first_use/pinned`, `seen`, `mismatched`), `ServerIdentity::load_or_create`, `read_message`. quinn's own congestion control is replaced by a fixed window (`ApplicationPaced`); the app pacer owns sending rate.
- Tests live in each crate's `tests/` (fec, frames, quic loopback; wire round trips; rate-control's congestion, loss and pacing).
