# Rikka Remote Protocol, version 1

A client shows and drives one desktop session of a Rikka machine. This document is the
whole wire contract; any client that follows it interoperates with `rikka-remote`.

## Transport

- QUIC version 1 (RFC 9000) with the unreliable DATAGRAM extension (RFC 9221) on UDP,
  port 47900 unless the host was configured otherwise.
- ALPN `rikka-remote/1`. TLS 1.3 only.
- The server presents a self-signed ECDSA P-256 certificate. The client trusts it on
  first use and pins the SHA-256 digest of the certificate's DER-encoded
  SubjectPublicKeyInfo per host. A changed digest is a refusal the user must confirm.
- The client sends QUIC PINGs every second; either side closes after 10 s of silence.
- All integers are little-endian. `str` is a `u16` byte count followed by UTF-8.
  `bytes` is a `u32` byte count followed by the bytes. `bool` is one byte, 0 or 1.
- Clocks: every `_us` field is microseconds of the sender's monotonic clock, truncated to
  32 bits; the receiver unwraps them.

## Streams

| Stream | Opened by | Content |
|---|---|---|
| first bidirectional | client | control messages, both ways |
| unidirectional, first byte `0x01` | client | input events |
| unidirectional, first byte `0x02` | client | one file upload |

### Control messages

Each message is `u32 length`, then `u8 type`, then `length - 1` bytes of body; a
message is at most 16 MiB. A receiver skips types it does not know.

Client to server:

| Type | Name | Body |
|---|---|---|
| `0x01` | Hello | `u16 version`, `str device`, `str platform` |
| `0x02` | SignIn | `u8 method`; method 0: `str password`, `bool remember`; method 1: `bytes token` |
| `0x03` | Start | `Viewport`, `u8 count`, `count × Decoder`, `u8 flags` (bit 0: send sound) |
| `0x04` | Viewport | `Viewport` |
| `0x05` | Keyframe | — |
| `0x06` | PointerLock | `bool locked` |
| `0x07` | Clipboard | `str mime`, `bytes data` |
| `0x08` | Ping | `u64 id` |
| `0x09` | SignOut | — (the server forgets the token this connection signed in with) |

Server to client:

| Type | Name | Body |
|---|---|---|
| `0x81` | Welcome | `u16 version`, `str host`, `str user` |
| `0x82` | SignedIn | `bytes token` (empty when not remembered) |
| `0x83` | Refused | `u8 reason`, `u32 retry_after_ms`, `str detail` |
| `0x84` | Display | `str name`, `u32 width`, `u32 height`, `u32 refresh_mhz` |
| `0x85` | Stream | `u16 stream`, `u8 codec`, `u8 chroma`, `u8 depth`, `u32 width`, `u32 height`, `u32 frame_rate_mhz`, `str encoder` |
| `0x86` | CursorShape | `u32 serial`, `u16 width`, `u16 height`, `i16 hotspot_x`, `i16 hotspot_y`, then `width × height × 4` bytes of straight-alpha RGBA to the end |
| `0x87` | Cursor | `u32 serial` (0 hides the cursor) |
| `0x88` | Clipboard | `str mime`, `bytes data` |
| `0x89` | Stats | `u64 target_bps`, `u64 estimate_bps`, `u32 frame_rate_mhz`, `u32 encode_us`, `u16 protection_permille`, `u16 loss_permille`, `u32 rtt_us` |
| `0x8A` | Pong | `u64 id` |
| `0x8B` | Ended | `u8 reason` |
| `0x8C` | FileSaved | `u32 id`, `str name` |
| `0x8D` | FileFailed | `u32 id`, `str reason` |

`Viewport` is `u32 width`, `u32 height` (physical pixels of the area showing the
desktop) and `u32 refresh_mhz`. `Decoder` is `u8 codec`, `u8 chroma` (mask: bit 0
4:2:0, bit 1 4:4:4), `u8 depth` (mask: bit 0 8-bit, bit 1 10-bit), `u8 reserved`,
`u16 max_width`, `u16 max_height`; decoders are listed in the client's order of
preference.

Codecs: 1 H.264, 2 H.265, 3 AV1. Chroma in Stream: 1 4:2:0, 2 4:4:4.
Refusal reasons: 1 wrong password, 2 unknown token, 3 too many attempts (wait
`retry_after_ms`), 4 sign-in unavailable. End reasons: 1 replaced by another viewer,
2 the host stopped, 3 signed out, 4 disconnected by the host, 5 unsupported version.

Cursor serials start at 1 and are never reused. The client keeps at least the 32
most recently defined or shown shapes; the server sends a shape again, under a new
serial, once it has fallen out of its own list of the 32 most recent.

Clipboard text travels as `text/plain;charset=utf-8`, at most 1 MiB; either side
ignores larger or non-UTF-8 text. In Stats, `estimate_bps` is the throughput the
client acknowledged recently, 0 until it is known. The server encodes no larger
than the `max_width` and `max_height` of the decoder it chose.

### Session

1. The client opens the control stream and sends Hello. The server answers Welcome,
   or Ended (5) when it does not speak the version.
2. The client sends SignIn until the server answers SignedIn. Refused leaves the
   connection open for another attempt.
3. The client sends Start. The server answers Display, Stream and the cursor state,
   then sends media datagrams. A new Stream (another `stream` number) follows every
   change of codec or picture size; the next frame of a new stream is intra.
4. Either side may close the connection; the server sends Ended first when it ends
   the session.

One viewer is live at a time: a new viewer ends the previous one with Ended (1).

### Input events

The stream starts with `0x01`; each event is `u8 type` and a fixed body:

| Type | Event | Body |
|---|---|---|
| `0x01` | Key | `u16 code`, `bool pressed` |
| `0x02` | Button | `u8 button`, `bool pressed`, `u16 x`, `u16 y` |
| `0x03` | Wheel | `i32 dx`, `i32 dy` in 1/120 of a notch |
| `0x04` | Scroll | `f32 dx`, `f32 dy` in desktop logical pixels |
| `0x05` | Relative | `f32 dx`, `f32 dy` in desktop logical pixels |
| `0x06` | Text | `str text` |
| `0x07` | ReleaseAll | — |

Positions `x`, `y` map 0..=65535 onto the shown display. Positive `dy` scrolls the
content up (towards the end of a document), positive `dx` towards its right.
Buttons: 0 primary, 1 middle, 2 secondary, 3 back, 4 forward.

`code` is the Linux evdev key code of the physical key, independent of the client's
keyboard layout; the desktop applies its own layout. A client forwards every key,
including system chords (Super/Windows, Alt+Tab, Ctrl+Alt+arrows), while its window
holds keyboard focus: Wayland clients through `zwp_keyboard_shortcuts_inhibit_manager_v1`,
Windows clients through a `WH_KEYBOARD_LL` hook. A key that cannot be captured locally
(Ctrl+Alt+Del on Windows) is offered as a menu action sending the same codes.
A client sends ReleaseAll when it loses focus.

PC set 1 scan codes map to evdev as follows: a plain scan code `s` in `0x01..=0x58` is
code `s`; extended (`E0`) codes map as

| Scan | Code | Key | Scan | Code | Key |
|---|---|---|---|---|---|
| `E0 1C` | 96 | keypad Enter | `E0 4D` | 106 | Right |
| `E0 1D` | 97 | right Ctrl | `E0 4F` | 107 | End |
| `E0 35` | 98 | keypad / | `E0 50` | 108 | Down |
| `E0 37` | 99 | Print Screen | `E0 51` | 109 | Page Down |
| `E0 38` | 100 | right Alt | `E0 52` | 110 | Insert |
| `E0 47` | 102 | Home | `E0 53` | 111 | Delete |
| `E0 48` | 103 | Up | `E0 5B` | 125 | left Super |
| `E0 49` | 104 | Page Up | `E0 5C` | 126 | right Super |
| `E0 4B` | 105 | Left | `E0 5D` | 127 | Menu |

and Pause (`E1 1D 45`) is 119, `0x70` (Katakana/Hiragana) 93, `0x73` (Ro) 89,
`0x79` (Henkan) 92, `0x7B` (Muhenkan) 94, `0x7D` (Yen) 124, `0x64..=0x6E` (F13–F23)
183–193, `0x76` (F24) 194, `0x72` / `0xF1` (Hanja) 123, `0x71` / `0xF2` (Hangul) 122.

### File uploads

A stream starting with `0x02` carries `u32 id`, `str name`, `u64 size` and then exactly
`size` bytes before its end. The server stores the file in the user's Downloads folder
under a free name and answers FileSaved or FileFailed on the control stream.

## Datagrams

The first byte is the type. Every server datagram carries `u32 seq` next: one counter
over all of the connection's server datagrams, increasing by one per datagram, used by
feedback.

| Type | Direction | Name |
|---|---|---|
| `0x01` | server → client | VideoShard |
| `0x02` | server → client | Audio |
| `0x03` | server → client | Padding |
| `0x04` | server → client | Pointer |
| `0x41` | client → server | Motion |
| `0x42` | client → server | Feedback |

### VideoShard

| Offset | Size | Field |
|---|---|---|
| 0 | 1 | type `0x01` |
| 1 | 4 | `seq` |
| 5 | 2 | `stream` |
| 7 | 4 | `frame`, increasing by one per frame of the stream |
| 11 | 4 | `reference`, the frame this one predicts from (equal to `frame` when intra) |
| 15 | 1 | flags: bit 0 intra, bit 1 refinement of the previous picture |
| 16 | 4 | `capture_us` |
| 20 | 4 | `frame_bytes` |
| 24 | 4 | `block_offset` |
| 28 | 2 | `block` |
| 30 | 2 | `blocks` |
| 32 | 1 | `k`, data shards of the block (1..=255) |
| 33 | 1 | `m`, parity shards of the block (`k + m` ≤ 255) |
| 34 | 1 | `shard`, 0..`k + m` (data shards first) |
| 35 | 1 | reserved, 0 |
| 36 | 2 | `shard_bytes` `S` |
| 38 | `S` | payload |

A frame is one access unit of the stream's codec: Annex B byte stream for H.264 and
H.265, a low-overhead OBU sequence for AV1. It is cut into `blocks` blocks; block `b`
holds bytes `block_offset .. block_offset + min(k × S, frame_bytes − block_offset)`,
and data shard `j` its bytes `j × S ..` zero-padded to `S`. Parity shard `i` of a block
is `Σⱼ c(i, j) · dⱼ` over GF(2⁸) with the polynomial x⁸+x⁴+x³+x²+1 (`0x11D`), where
`c(i, j) = 1 / ((k + i) ⊕ j)`: a Cauchy matrix, so any `k` shards of a block restore it.

A client decodes a frame only when it is intra, or when its `reference` was decoded.
Frames it cannot decode are never shown: the client keeps the last good picture until
the server sends a frame predicting from a frame the client reported good, or an intra
frame. A refinement frame improves the picture already shown.

### Audio

`u8 type`, `u32 seq`, `u32 timestamp` (48 kHz samples), `u8 flags`. With flags bit 0
clear the rest is one 10 ms stereo Opus packet. With bit 0 set, `u16 length` and that
many bytes of Opus packet follow, and the rest is the previous packet again
(timestamp − 480), for the client to recover a single loss.

### Padding

`u8 type`, `u32 seq`, then zeros. The server probes the path's capacity with it.

### Pointer

`u8 type`, `u32 seq`, `i32 x`, `i32 y`: the desktop's pointer position in display
pixels, sent while the client holds a pointer lock and draws the cursor itself.

### Motion

`u8 type`, `u16 seq`, `u16 x`, `u16 y`: an absolute pointer position (0..=65535 over
the display). The server drops a motion whose `seq` is not newer than the last one.

### Feedback

The client sends feedback every 25 ms for as long as the session runs, and at once
when a frame is lost.

| Size | Field |
|---|---|
| 1 | type `0x42` |
| 4 | `base_seq`, the first `seq` this feedback covers |
| 2 | `count` of covered `seq` values |
| 4 | `base_us`, arrival of the first received datagram covered |
| ⌈count / 8⌉ | received bitmap; bit `i % 8` of byte `i / 8` is `base_seq + i` |
| 2 × (received − 1) | `i16` arrival deltas in 50 µs units from the previous received datagram |
| 2 | `stream` the frame numbers below belong to |
| 4 | `good`, the newest frame of `stream` the client decoded (`0xFFFFFFFF` for none) |
| 1 | `lost` count |
| 4 × lost | frames of `stream` the client gave up on |

Consecutive feedbacks cover consecutive ranges. A lost frame is reported in three
consecutive feedbacks. The server ignores frame reports about another stream than the
one it sends.
