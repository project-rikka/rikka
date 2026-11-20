+++
name = "Write"

[params]
path = "An absolute path inside the working folder, or one relative to it."
content = "The file's complete contents."
+++

Creates or atomically replaces a whole file and any missing parent directories. Use `{{edit_tool}}` for partial changes.

- Replacements overwrite without a revision check and preserve existing line endings and permissions.
