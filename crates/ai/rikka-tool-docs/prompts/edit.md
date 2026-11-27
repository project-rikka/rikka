+++
name = "Edit"

[params]
path = "Relative to the working folder."
edits = "Changes to the file, each setting exactly one of `lines`, `after` or `before`; all apply or none do."
lines = "Anchor of the one line to replace, or anchors of the first and last line of a range."
after = "Anchor of the line the text goes after."
before = "Anchor of the line the text goes before."
text = "The new lines, without anchors; empty deletes the lines. A final newline is ignored."
+++

Changes lines of an existing file by their anchors; `{{write_tool}}` creates or replaces whole files.

- An anchor is a line number with its tag, like `12#ab`. Numbered listings of working-folder files from `{{bash_tool}}` (`cat -n`, `rg -n`, `grep -n`) and every edit result print it before each line; copy it from there, never make one up.
- `text` is exactly what the lines become or what goes in: leave out the kept lines beside it, including an `after` or `before` line, and write every blank line the result needs.
- Make every change to a file in one call. All its anchors refer to the file before the call, so edits never shift each other, but they may not overlap. Lines below a change move: take later anchors from its result or a fresh listing.
- A refusal writes nothing and shows the lines involved with current anchors: a stale or mistyped anchor, overlapping edits, a kept line repeated, or new syntax errors. Fix the call from it; do not reread the file.
- The result shows each changed place with two lines either side and fresh anchors; check every seam there. Line endings and byte-order mark are kept.
