---
description: Review changed code for reuse, quality, and efficiency, then fix worthwhile issues.
argument-hint: "[path or area to narrow to]"
---

# Simplify changed code

Review the changed code for reuse, quality, and efficiency; fix every worthwhile issue without changing intended behavior.

Requested scope: $ARGUMENTS

If no scope is given, inspect all staged, unstaged, and untracked changes. If none exist, inspect the latest files named by the person or edited in this conversation.

Use one `Agent` as an independent reviewer. Give it a self-contained scope and require it to inspect the changes, neighboring code, and project conventions, then return only actionable findings with `file_path:line_number` and a recommended fix.

Review for:

- **Reuse:** duplicated functions or logic; new code that should use existing types, utilities, helpers, or shared modules.
- **Quality:** redundant or derivable state; parameter sprawl; near-duplicates; leaky abstractions; stringly-typed values; layout-neutral UI wrappers; deeply nested branches; comments that restate code, the task, or the change rather than a hidden constraint or invariant.
- **Efficiency:** repeated computation, reads, or network calls; N+1 work; missed concurrency; blocking hot paths; unconditional no-op updates; ignored no-change signals from updater callbacks; TOCTOU existence checks; unbounded data, missing cleanup, or listener leaks; operations broader than needed.

Validate every finding against the code. Fix real issues directly; skip false positives and unjustified refactors. Remove resulting dead code, run relevant formatting and tests, and verify runnable behavior with `Computer` when applicable.

Finish with a brief summary of fixes and verification, or confirm that the reviewed code was already clean.
