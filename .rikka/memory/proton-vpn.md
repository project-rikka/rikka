---
title: Proton VPN client
description: rikka-proton-vpn picks servers, gets certificates, connects through NetworkManager and runs the in-tunnel local agent.
about: crates/proton/rikka-proton-vpn
tags: [proton, vpn]
---

- `VpnClient::new(api)` needs no key hierarchy; credentials ride the session. It depends on rikka-network and rikka-proton-account and uses rustls directly (the agent).
- `KeyPair` matches Proton's Go shapes (seed, PKIX DER, base64, PEM); `x25519_secret_key()` is the clamped first half of SHA-512(seed); `encrypted_private_key_pem(passphrase)` writes PKCS#8 PBES2 (PBKDF2-SHA256, AES-256-CBC) for OpenVPN.
- `CertificateRequest` → `vpn/v1/certificate`; `Certificate::freshness_at` answers Fresh, Stale or Expired.
- Servers: `logicals().send()` → `ServerList::{Changed { logicals, last_modified }, Unchanged}` (If-Modified-Since/304). `status(id)` decodes `vpn/v2/status/{id}/binary`; `Scoring::for_user(&location).apply` recomputes load and score. `ServerQuery` chooses like the official mobile client (score, tier, country/city/state, secure core, P2P/Tor/streaming, gateways) with `fallbacks()`.
- Local agent (`agent/`): mutual TLS to 10.2.0.1:65432 with the session's ed25519 certificate, server name checked against the embedded roots (`src/agent/roots.pem`); length-prefixed JSON status/features/errors; go-vpn-lib's states, error codes, backoff and keepalive.
- `Connector<T: TunnelControl>` (`NetworkManager::system()` is the real one): certificate → `TunnelPlan` (WireGuard UDP, OpenVPN UDP/TCP in certificate mode with the Linux client's CA and tls-crypt key, or WireGuard TCP/TLS through Proton's `protun` NM plugin when installed) → tunnel → agent. `VpnConnection` and `AgentConnection` publish desired `Controls` (settings/features, connectivity, status-request counter, disconnect) through `watch` channels instead of queues; agent events use a bounded channel. Agent errors get the official mobile client's reactions (`connect/reaction.rs`); certificate renewal rewrites OpenVPN cert files via `TunnelControl::renew`.
- Sign-in codes: `sign_in_code()` → `SignInCode::url`, `poll_sign_in`/`await_sign_in` (pending is HTTP 422), `web_sign_in_url`.
- Default ports are the Linux client's (WireGuard first 443).
