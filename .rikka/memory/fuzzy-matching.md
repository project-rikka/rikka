---
title: Fuzzy matching (rikka-fuzzy)
description: rikka-fuzzy is the one fuzzy scorer: Needle, Haystack, Score, positions, best/ranked; launcher, completion, ADE.
about: crates/foundation/rikka-fuzzy
tags: [search, foundation]
---

- A public, dependency-free crate. `Needle::new(text)` folds case; `Needle::with_smart_case(text)` matches an atom exactly when it has an uppercase letter.
- A needle is whitespace-separated atoms; every atom must match as a subsequence, in any order, and the score is their sum.
- `Haystack::new(text)` folds once and precomputes bonuses (after `/`, word separators, `.`, camel humps); keep it per candidate set.
- `needle.score(&str)` / `find(&Haystack)` answer `Option<Score>`; `positions(&Haystack, &mut Vec<usize>)` also fills the matched char indices of the scored alignment, and `byte_ranges(text, positions)` turns them into highlight ranges.
- `best(candidates, limit)` keeps the top scores in input order for ties; `ranked` sorts all. `Score::new`, `NEUTRAL`, `scaled`, `offset` let callers add their own classes.
- Haystacks or atoms over 256 chars get a flat length penalty and leftmost positions instead of the alignment.
- Users: the shell launcher, `rikka_syntax::Lexicon::suggest`, `ade_paths::Paths`.
