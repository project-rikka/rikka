---
description: Cut low-value, duplicate, or implementation-coupled Rust tests and the test-only production seams they keep alive; gate new tests against the same bar.
argument-hint: "<path, crate, or area> | --author <test path>"
---

# Test audit

Scope: $ARGUMENTS

Optimize for confidence per line of maintained code, not deletion count. With `--author`, apply only the [authoring gate](#authoring-gate) to the named new or changed tests. Otherwise audit the scope; if it is missing or ambiguous, ask.

## Value bar

A test earns its cost only by protecting observable behavior, a credible regression, or an independent contract. Each contract has one primary test at its strongest boundary (public API, protocol, wire or disk format). Another layer needs a risk that owner cannot reach, such as a lifecycle, transport, or concurrency failure.

A test that breaks under a behavior-preserving refactor asserts implementation. When writing, rewrite it at the owning boundary. When auditing, treat it as suspect, not yet deletable.

## Authoring gate

Every new or changed test must answer all four, or it is not added:

1. Which behavior, invariant, or contract does it protect?
2. Which credible regression makes it fail?
3. Why doesn't existing coverage already catch it? Extend an existing table of cases or shared fixture instead of adding a near-duplicate.
4. Does it need a production seam that no production caller uses? Then test at the real boundary instead.

It must also match no [junk pattern](#junk-patterns) unless the [retention bar](#retention-bar) names the contract it alone guards. A regression test must fail on the pre-fix code for the intended reason (revert the fix locally and run it) and live at the owner boundary only, not at every layer the bug crossed.

## Junk patterns

General:

- no assertion, or an assertion that cannot fail (`assert!(true)`, `let _ = f();` as coverage);
- self-comparison, or an expected value computed by the code under test;
- copied fixtures, inventories, or export lists that mirror the source;
- the same contract asserted again in a sibling test, in both a unit and an integration test, or by a per-implementation replay of a shared helper;
- private predicates or call shapes also covered through the public path;
- mocks that implement the asserted behavior, or one mock standing in for different APIs;
- fixtures that supply the ordering, acknowledgement, or state the owner should produce, or persistence checked against a store the path never writes;
- negative controls that pass for an unrelated reason (a different guard rejects, or production never reaches that input);
- names that promise more than the input exercises;
- sleeps or wall-clock timing standing in for synchronization.

Rust-specific:

- restating what the compiler or a derive already guarantees: `Clone`, `Default`, `PartialEq`, `Ord`, `Hash`, or `Debug` derives, trivial `From`/`new`/getters/setters, enum discriminants, field access;
- runtime checks of static facts (`Send`, `Sync`, sizes, trait impls). Use a `const` assertion or delete them;
- `#[should_panic]` without `expected = "…"`, or `assert!(r.is_err())` without matching the error variant: both pass on any failure;
- `Debug`/`Display` string equality where the text is not a user-facing contract;
- serde round-trips of plain derives with no custom logic and no persisted or wire format;
- `#[ignore]` tests that no CI or `rime test --machine` run reaches;
- tests duplicating a doc test that already runs.

Test-only seams, deleted together with their tests:

- `#[cfg(test)]` methods, fields, or impls on production types;
- `pub`, `pub(crate)`, or `#[doc(hidden)] pub` widened only so tests can reach it;
- `test-support`/`testing` features, `*_for_test` constructors, and dev-dependencies used only by removed tests;
- traits with one production impl that exist only to be mocked;
- `#[allow(dead_code)]` or `#[cfg_attr(not(test), allow(dead_code))]` hiding code only tests call.

## Retention bar

Keep a test that independently enforces a public crate API, Wayland or IPC protocol behavior, a wire, file, or config format, migration, storage, security or privilege boundary, `unsafe`/FFI invariant, locale catalog, default, or release or packaging contract. Also keep:

- call ordering that is observable;
- regressions with a credible failure mode;
- concurrency, cancellation, and drop-order tests;
- property or fuzz-style tests that cover input spaces examples miss;
- a source or string check that is the cheapest guard of a user-facing key, byte, or path and survives identifier renames.

Static, slow, or machine-only (`tests/machine.rs`) is not a reason to delete. A retained test that fails on the baseline is a possible product bug: reproduce it and fix the owner, don't delete it.

## Discovery (read-only)

1. Read `RIKKA.md` and the relevant `.rikka/memory` notes, then the crate's `Cargo.toml` (`[[test]]` targets, `[dev-dependencies]`, `[features]`) and any `rime.toml` override naming it.
2. For each candidate, read the whole test, the production owner, its callers and callees, sibling implementations, overlapping tests, and `git log` for why it exists. Read dependency source when the test claims dependency behavior.
3. Find seams: `rg -n '#\[cfg\(test\)\]|cfg_attr\(not\(test\)|allow\(dead_code\)|doc\(hidden\)|feature = "test' <scope>`. Then check each hit's non-test callers with `rg`.
4. For a large scope, split disjoint areas (core crates, public crates, system daemons, apps, a cross-cutting pattern sweep) across `Agent`s with this document. Merge only evidence-backed candidates. Prefer a few certain candidates over a long speculative list.

Record, per candidate, before editing. A missing field means it stays:

- test name and `file_path:line_number`;
- the failure it can actually detect;
- the stronger remaining proof, or why none is needed;
- non-test callers of any seam it uses;
- history: why it or its seam exists;
- production code, features, or deps its removal unlocks.

## Edit

- Take one coherent batch per owner boundary. Delete tests, seams, and dead production paths together; don't leave aliases or `#[allow(dead_code)]`.
- Move retained regressions to their owner. Merge near-duplicates into one table of cases.
- Remove dev-dependencies, features, `[[test]]` entries and `rime.toml` overrides that no longer apply. Deleting `tests/machine.rs` means deleting its `[[test]] name = "machine"` entry too, or the manifest names a missing file.
- Prefer net-negative production code. Add no replacement test that restates the same implementation, and don't pad the batch with uncertain candidates.

## Validation

1. `rime test <scope>` plus the consumers of any changed API or feature (scoped runs miss reverse deps), and `rime test --machine <scope>` when the package has a `tests/machine.rs`.
2. `rime clippy <scope> --all-targets -- -D warnings`: removed tests often expose newly dead code; delete it rather than allowing it.
3. `cargo fmt --all --check` and `git diff --check`.
4. For a regression moved or rewritten, show that it still fails on the pre-fix code.
5. `git diff --numstat`, split into production code and tests.

Don't commit or push unless asked.

## Report

- removed categories, with counts;
- production seams and dead code removed;
- candidates kept and the contract each guards;
- commands run and their results;
- production versus test line deltas;
- follow-up batches, highest confidence first.
