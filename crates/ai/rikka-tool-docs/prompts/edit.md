+++
name = "Edit"

[params]
path = "Relative to the working folder."
old_string = "Exact text to replace, indentation included; it must occur once unless `replace_all` is set."
new_string = "Its replacement; empty deletes it."
replace_all = "Replace every occurrence."
edits = "Replacements applied in order, each to the result of the one before; use instead of `old_string` and `new_string`."
+++

Replaces exact text in an existing file; `{{write_tool}}` creates or replaces whole files.

- Copy `old_string` from what `{{bash_tool}}` printed or your last edit's result, without line numbers, with just enough neighbouring lines to be unique. `new_string` replaces all of it, so repeat the neighbours that stay.
- Make every change to a file in one call, through `edits`; all apply or none do.
- A refusal writes nothing and explains itself: missing text comes with the closest lines, repeated text with its lines, and a `new_string` that repeats a line kept beside it or adds syntax errors with the lines involved. Fix the call from that reply rather than rereading the file.
- The result shows each changed place renumbered, with two lines of context; check every seam. Line endings and byte-order mark are kept.
