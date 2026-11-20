---
title: Proton Key Transparency
description: rikka-proton-key-transparency verifies others' keys and self-audits SKLs against Proton's KT log over the CT list.
about: crates/proton/rikka-proton-key-transparency
tags: [proton, keys]
---

- `KeyTransparency::builder(api).pgp(..).realm(..).log_lists(..).state(..).persist(..).build()`; one per session. `KeyTransparency::for_account(account, log_lists)` builds it over the account's API and PGP and persists the sealed `AuditState` in the account cache's `key-transparency.audit-state`. Without `log_lists` it gets an uncached Chrome-sourced `LogLists`.
- Others' keys: `kt.lookup(email).skipping_excluded_domains().send()` → `VerifiedPublicKeys` (keys come from `Account::public_keys`; KT exposes no key types of its own), `kt.check(email)…verify()`, `verify_keys`. Verdicts: `Verified`, `Pending`, `Unverifiable`, `Failed(Failure)`; transport problems are `Error`.
- Self-audit: `kt.audit(&AccountKeys)` → `AuditReport` when `is_audit_due()`; retries back off 2^n minutes up to 4 h, 6 tries. After publishing an SKL callers call `record_published`; `resign_history` after a primary change, `reset_audit` after a password reset.
- ECVRF is draft-10 EDWARDS25519-SHA512-TAI on curve25519-dalek 5. Proof path = first 28 bytes of the VRF output + 4-byte big-endian revision; leaf = `SHA256(SHA256(content) ‖ MinEpochID)`.
- Epochs: chain hash, certificate alt name `<CH[..32]>.<CH[32..]>.<time>.<epoch>.1.<domain>`, X.509 chain to pinned LE/ZeroSSL roots (webpki + aws-lc-rs), then `LogList::embedded_scts` from rikka-certificate-transparency must hold at least one SCT from a currently qualified log (Proton's one-operator threshold). `Epoch::verify` and `EpochCertificate::verify` take the `&LogList`. An `InsufficientTimestamps` result refreshes a due list once and re-verifies. HTTP 422 on a proof means a stale epoch (one retry).
- `data/` holds only roots and excluded domains, which carry no date limit. The CT log list lives in rikka-certificate-transparency; KT re-exports `LogLists`, `LogList`, `LogListSource`, `LogListKey`, `ListUpdate` and `LogListError`.
- `is_available().await` and every entry point go through `LogLists::fresh`: an expired list (70 days) is refreshed first, otherwise `Error::LogList(LogListError::Expired)`. `refresh_log_list()` refreshes when due; its `Feed` impl calls it when a self-audit is due. `ProtonBuilder::log_lists` sets the shared lists; `ProtonBuilder::accounts(root, ..)` defaults to lists cached in `<root>/certificate-transparency`, apps/system/proton passes `AccountStore::log_lists()` cached in `<cache>/rikka/certificate-transparency`.
- Time only comes from the injected `Pgp` clock.
