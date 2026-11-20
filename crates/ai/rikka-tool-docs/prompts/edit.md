+++
name = "Edit"

[params]
path = "The file path, relative to the working folder."
ops = "Operations numbered against the same listing of the file; they never overlap, and all apply or none do."
replace = "Makes `start_line` through `end_line` read exactly `text`."
insert_before = "Puts the lines of `text` before `line`, keeping that line."
delete = "Removes `start_line` through `end_line`."
start_line = "First line replaced, copied from a numbered listing."
end_line = "Last line replaced, copied from a numbered listing; never before `start_line`."
line = "The line the text goes before, copied from a numbered listing; one past the last line adds at the end."
replace_text = "The new lines joined by newlines. A final newline makes an empty last line; empty text removes the range."
insert_text = "The new lines, each ending in a newline; a missing final one is added."
+++

Changes an existing text file by line number. Use `{{write_tool}}` to replace a whole file.

- Copy every number from a numbered listing of the file: `cat -n` or `rg -n` through `{{bash_tool}}`, or the result of your last edit to it. Never count lines, compute a number, or take one from unnumbered output such as `sed -n` or `head`.
- `text` is exactly what the range becomes. The line above `start_line` and the line below `end_line` stay, so `text` never begins with the one or ends with the other; to keep a closing bracket, end both the range and the text before it.
- Cover only lines that change. To add lines after line N, insert before N + 1 instead of widening a `replace` to reach them.
- Blank lines are lines: write each one `text` needs, such as the blank between a new item and its neighbour.
- Put all changes to a file in one call, numbered against one listing. An edit renumbers every line below it, so take later numbers from its result or from a fresh listing.
- An edit whose text repeats a line kept beside its range, or that leaves code with syntax errors it did not have, is refused with the lines involved and nothing is written; correct it and send the whole edit again.
- The result shows each changed place in the new numbering, two lines either side; a `…` row skips unchanged lines or the middle of a long change, and a very long result is cut short. Check every seam shown.
- Line endings and byte-order mark are preserved.
