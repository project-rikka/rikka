---
title: Packages application
description: Browses and searches a local nixpkgs catalog and stages declarative package and module changes into one rebuild.
about: apps/system/packages
tags: [application, nix]
---

- `nixpkgs-catalog` owns the catalog file format, import of a channel `packages.json.br`, search and `Package`/`License`/`Maintainer`/`Team`. `nixpkgs-search` re-exports them and owns finding and downloading releases, Nix evaluation and the option index, not this machine. The window owns declared intent, generations and the pin.
- Views: Search (every package by attribute when the query is empty), Installed (reads `/etc/nixos/rikka/packages.nix`), Apps (Rikka's own components from `rikka_nix::Distribution`, toggled into the `apps` module through `apps.rs` `Apps`, applied in the same plan), Generations.
- `CatalogCache::new(folder).open(pin, cancel, report)` returns the `Catalog`. `platform::pin()` builds the `Pin` from `Channel::detect()` and `rikka_nix::running_version()`; the folder is `Paths::cache_dir()/packages`. The window reopens the cache when a finished job changes the pin.
- Opening finds a cached `<pin commit>.catalog` without network. Otherwise it lists `nix-releases` S3 for the release whose short commit matches, falls back to the channel head only when the release is not listed or the bucket answers 404, downloads `packages.json.br` (compression off, resumable) and imports it. On any other failure (offline included), the newest cached catalog is used.
- A catalog file is `BinaryFormat` `RKNIXPKG`, a u64 index length and a `Codec` index (text arena and spans for list fields), then zstd blocks of 64 JSON details read on demand. Corrupt files are refused (`Error::Read`) and rebuilt. Sorting, `find` and search share one case fold (`search_index::fold_case`).
- Search is in-process over a lowercased haystack (memchr per word, most selective first) and ranks names before description mentions. It runs keyed in `spawn_blocking`, so newer queries drop older ones.
- `Nixpkgs::lookup` evaluates only attributes missing from the catalog (aliases such as `python3Packages.*`).
- Install and remove edit one staged `included` set (`Plan::include`/`exclude`). Apply sends the edited modules through `rikka-nix` as one authorization and one rebuild, based on the `Modules` the plan was read from (a `Stale` refusal rereads them and `Plan::declared` keeps the staged adds and removes), with no cancel (rollback via generation). `Work` pairs the `Job` with a `rikka-nix` `Rebuild`; the bus ask runs in `spawn_blocking`. While a job runs, its button (apply, update, roll back) shows `pane::working(stage)` and the others are disabled; progress, log and result live in the shell notification and `rikka-updates`, and refusals other than `Denied` show as a strip.
- Module integration is inferred from `optionsJSON` defaults. `Options::requirements(attribute, unfree)` returns `Suggestion`s (`Requirement::{Option, Group, Unfree}`, `recommended`) and suggests same-name `programs.*` modules, which replace list entries only when they guarantee the executable. Service modules are offered, not assumed.
- Unfree enables `allowUnfree` and never silently disables it. The generated file is the only record.
- Rows are `listing::Row` (catalog `Id`, described `Package` or unknown attribute) shared as `Arc<Vec<Row>>`; marks are a separate small map. Cells are self-painted in a lazy collection.
- `nixpkgs_search::Cancel` owns the running `nix` and the download token. Each app `CancelGuard` stops its run when renewed or dropped.
- `nixpkgs-catalog` tests import `tests/packages.json` (brotli-compressed in the test). `nixpkgs-search` tests put a scripted `nix` first on `PATH` (`tests/support`) for lookups; nix tests take turns on one lock.
- `manager/` holds window state: `Index::{Building(stage), Ready(catalog), Failed}`, `Backend` (nix for lookups), `Search` (query, found rows, nix descriptions), `ListingView` (listing, selection, selected package) and `view.rs`.
- `configuration.rs` owns the option paths the app reads and writes: `Declared::read` and `Changes::request` (built by `Plan::changes`) compose the full module set for one apply.
