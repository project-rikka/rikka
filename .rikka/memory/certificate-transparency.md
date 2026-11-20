---
title: Certificate Transparency log list and SCTs
description: rikka-certificate-transparency fetches and verifies Chrome's signed CT log list, caches it, and checks embedded SCTs.
about: crates/foundation/rikka-certificate-transparency
tags: [security, certificates]
---

- Internal crate; its only consumer is rikka-proton-key-transparency, which re-exports it through the Proton facade.
- Only Chrome's log list signing key ships (`data/log_list_pubkey.pem`, `LogListKey::chrome()`); it is the trust anchor and must never be fetched. The list itself is fetched at run time; nothing else is bundled.
- `LogList::from_signed(json, sig, &LogListKey)` verifies the exact bytes before parsing Chrome's v3 schema (`log_list.json`, RSA-4096 PKCS#1 SHA-256 `.sig`; `LogListKey` also takes P-256 SPKI keys). Entries whose `log_id` is not SHA-256 of the key, with unknown keys or with not exactly one state are skipped.
- A list is fresh for 70 days after `log_list_timestamp` (Chrome's and Proton's limit).
- `LogLists` (`builder().source(LogListSource).cache_dir(dir).build()`) starts from the cached list, if any (`current()` is `None` until one is fetched). `fresh(now)` fetches a missing or expired list before answering (`Error::NotFetched` / `Error::Expired`), `refresh_if_due(now)` fetches once the list is a day old.
- Fetches use no transport retries; failures back off from 15 min doubling to a day (an hour while no fresh list exists); one refresh runs at a time; older or equal lists never replace the current one. The cache file is a JSON envelope of base64 list and signature, re-verified on load and adopted from other processes before fetching.
- `embedded_scts(leaf_der, issuer_spki_der)` verifies RFC 6962 precert SCTs from logs that were ever qualified and keeps those whose log is qualified, usable or read-only (`Standing::Current`) or retired after the earliest such SCT (`Standing::Retired`); pending and rejected logs never count. The caller applies its own count policy.
- Tests use `rikka-test-certificate-transparency` (`testonly`): a byte-exact Chrome-signed list fixture (`chrome_log_list`, `CHROME_LIST`/`CHROME_SIGNATURE`), `log_list_at`/`log_list_with_states` edits of it, a P-256 `Signer`, and a thread-based `LogListServer`; `serving(timestamp)` gives a `LogLists` already pointed at a local server, so no test reaches Google.
- Chrome's list is published for CAs and CT monitors/auditors; Chrome's policy forbids relying on it for CT enforcement in TLS clients other than Chrome without permission, so never wire it into TLS verification.
