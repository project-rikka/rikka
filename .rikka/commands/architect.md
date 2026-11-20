---
description: Improve Rust architecture and API design with simple internals and expressive high-level calls.
argument-hint: "<path, crate, or area>"
---

Improve the Rust architecture within: $ARGUMENTS

If the scope is missing or ambiguous, ask before proceeding. Read the scoped code, its dependencies, callers, tests, and project conventions before proposing changes.

Before editing, use `AskUserQuestion` to agree on architecture and API design. Group consequential choices, recommend the simplest coherent design, and present concise, high-density before-and-after comparisons of key APIs and representative call sites with trade-offs. Resolve new design questions before implementing them; discover routine facts yourself.

- Breaking API changes and ground-up rearchitecture are valid options within scope. Prefer a coherent design over a minimal diff; agree on the extent and trade-offs before implementation.
- Hide low-level mechanisms behind intention-revealing APIs. Keep internals equally simple; moving complexity behind a facade is not enough.
- Give each module one responsibility, keep dependencies one-way, and replace repeated orchestration with cohesive operations.
- Prefer concise, expressive, idiomatic Rust over cleverness or fewer lines alone. Make common calls natural and invalid states hard to express.
- Minimize symbolic noise in public APIs and call sites: prefer fewer `<>`, `_`, and `'` markers through type inference, simple names, and encapsulated generics and lifetimes. Preserve idiomatic Rust naming, clarity, and type safety.
- Use traits and generics to clarify contracts or remove duplication. Also propose forward-looking flexibility and abstraction layers when useful; explain benefits, added complexity, and maintenance costs, and agree before introducing them.
- Use macros or proc-macros only as a last resort when ordinary Rust cannot achieve comparable clarity and they materially simplify the whole codebase. Explain the benefit and maintenance cost, and ask before introducing them.

Implement the agreed design completely, preserving intended behavior unless a change is approved. Update affected callers and tests, remove obsolete code, and avoid unrelated refactors. Run project-required formatting, lint, and tests, including affected consumers.

Finish with a brief summary of the design, verification, and any remaining blockers.
