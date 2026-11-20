---
title: FIDO2 security key client (rikka-fido)
description: rikka-fido is the workspace's CTAP2/U2F client over hidraw with WebAuthn client data, PIN, extensions and ceremonies.
about: crates/desktop/rikka-fido
tags: [security, hardware]
---

- Public crate (the browser may use it too). Entry: `Fido::new().assert(request)` / `.register(request)` → `Ceremony<T>` on a `rikka-fido` thread; read `Event`s (`Insert`, `Choose`, `Touch`, `Verify`, `Pin`, `NewPin`, `Account`), answer with `pin()`/`choose()`, `cancel()`, then `finish().await` or `wait()`. Dropping cancels.
- Devices: `/dev/hidraw*` whose report descriptor has usage page 0xF1D0; vanished devices are skipped; with several keys the touched one wins and the others are cancelled (`Watch::fork` gives the candidates one child `CancelToken`). With no key present the ceremony waits on a udev `hidraw` monitor (`hid/hidraw_monitor.rs`, `Transport::hotplug`); fake transports default to `StaticDevices`, which only wakes on cancel or deadline. Cancellation is `rikka_types::CancelToken` (`Ceremony::cancel_token`); `Watch::pause` blocks on `CancelToken::wait_timeout`. `HidLink::receive(wait, interrupt)` takes an optional fd that ends the wait early; CTAPHID replies pass the ceremony's `CancelSignal` (a pipe woken by the token's waker) until a CANCEL has been sent, so there is no polling slice.
- Transport: CTAPHID framing (INIT/CBOR/MSG/CANCEL/KEEPALIVE/ERROR, continuation packets, busy retries). CTAP2 GetInfo, GetAssertion (+ GetNextAssertion), MakeCredential, Selection; U2F-only keys fall back to CTAP1 with results converted.
- PIN protocols 1 and 2, tokens with permissions; a new PIN needs `max(4, minPINLength)` chars, ≤ 63 bytes. In a ceremony a missing PIN or `forcePINChange` asks through `Event::NewPin`; outside, `Authenticator::set_pin`/`change_pin`.
- Extensions: credProtect, credProps, hmac-secret (+ hmac-secret-mc), PRF (inputs hashed with the "WebAuthn PRF" prefix), minPinLength, largeBlob read. `AttestationConveyance` defaults to none (fmt `none`, zero AAGUID; packed self-attestation kept).
- WebAuthn side: exact `clientDataJSON`, rpId validated as a registrable domain suffix with `rikka-public-suffix` (IDNA to ASCII), AppID same-site by `registrable_domain`, AppID extension for legacy U2F registrations.
- `rikka_fido::cbor::Value` is the shared CBOR type (CTAP2 canonical order). `Fido::with_transport` takes `Transport`/`HidLink`; tests use `rikka-test-fido`'s `FakeKey`/`FakeKeys` (CTAP2.1/2.0/U2F, `cred_random`, `with_large_blob`, touch delay).
- Not implemented: large-blob writes and the CTAP 2.1 management commands (credential management, bio enrollment, authenticator config).
