---
title: SDK usage examples (sdk/examples/)
description: sdk/examples/<language>/ holds the only runnable examples: same programs in all six languages, glob-discovered targets.
about: sdk/examples
tags: [sdk, examples]
---

- `sdk/examples/<language>/` (`c`, `cpp`, `go`, `csharp`, `java`, `python`) are the workspace's only usage examples; there is no top-level `examples/` and no Rust example.
- Every language carries the same programs with the same behaviour and app id `org.rikka.examples.<name>`: `counter`, `music_player` (playlist, `on_end`, scrub state), `video_player`, `image_viewer` (chooser, drag and drop, `open_file`) and `dialogs` (`alert`, `confirm`, `open_url`).
- Sliders bound to a playing position keep a scrub value while dragged and seek on commit, because the view re-runs as playback advances.
- Each package's `BUILD.bazel` makes one target per file from a `glob`: `*.c`, `*.cpp`, `*.cs`, `*.java`, `*.py`, and Go `<name>/main.go` (all `.go` files of that directory). Adding an example means adding its file in every language.
- File names follow each language (`TextEditor.java`, `text_editor.py`); `example_name` in `//sdk/examples:defs.bzl` gives one snake_case target name, so every example runs as `bazel run //sdk/examples/<language>:<name>`.
- Rules: `rikka_cc_binary` (C11 / C++20, `-Wall -Wextra -Werror -pedantic`), `rikka_go_binary`, `rikka_csharp_binary` (top-level statements), `rikka_java_binary` (`main_class` = file stem, default package), `rikka_py_binary`; `//sdk/examples/python:types` runs `mypy --strict` over the Python examples.
- They need a Wayland session; `bazel test //...` only builds them. To check one on the `Computer` screen, `bazel run --script_path=<file> //sdk/examples/<language>:<name>` and launch that script.
