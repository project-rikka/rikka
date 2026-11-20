---
title: Audio processing port (rikka-apm crates)
description: rikka-apm facade over own audio-processing ports: band split, HPF, AEC3, NS, AGC2 with RNN VAD; oracle-tested.
about: crates/media/rikka-apm
tags: [audio, voice]
---

- Ported from freedesktop webrtc-audio-processing 2.1 (`reference/webrtc-audio-processing`, git-ignored), 48 kHz only, scalar code, no unsafe.
- `rikka-apm-audio`: `Frame` = reference `AudioBuffer` at 48 kHz (FloatS16 samples, 480 full-band, 3 × 160 bands via `ThreeBandFilterBank`); shared by the component crates.
- `rikka-apm-aec3` (`EchoCanceller`), `rikka-apm-ns` (`NoiseSuppressor`, `Level`), `rikka-apm-agc2` (`GainController`, adaptive digital + limiter + RNN VAD; adaptive is off by default as upstream) are internal.
- `Processor::clean`/`level` hold a `rikka_simd::FlushDenormals` guard (the reference's `DenormalDisabler`), so decaying AEC3 filters never hit denormal slow paths.
- Conformance tests pin golden values captured from the C++ library: nixpkgs `webrtc-audio-processing` 2.1 exports its internal classes; build harnesses with `nix shell nixpkgs#gcc` (the library needs a newer glibc than the dev shell) and abseil headers from nixpkgs. Overriding `webrtc::GetCPUInfo`/`GetAvailableCpuFeatures` in the harness (called through the PLT) forces its scalar path, against which AEC3, NS and AGC2 are bit-exact.
