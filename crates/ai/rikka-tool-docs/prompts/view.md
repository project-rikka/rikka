+++
name = "View"

[limits]
max_bytes = 3145728

[params]
path = "An absolute image path, or one relative to the working folder."
+++

Shows a PNG, JPEG, GIF or WebP picture for visual inspection; read text with `{{bash_tool}}`. Refuses files over {{max_bytes}} bytes.
