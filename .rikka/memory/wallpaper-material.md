---
title: Material source colour
description: rikka-theme-seed extracts and caches the generated-theme source colour of a picture or video.
about: crates/graphics/rikka-theme-seed
tags: [material, theme]
---

- `seed(path)` synchronously returns one Material source `Color`. Call it on a worker.
- It lives outside `rikka-theme` so rendering processes do not link decoders.
- It scores a 128×128 reduction and prefers opaque pixels (`rikka_material_color::quantize` into 128 clusters, then `score`). Videos use a software-decoded first frame.
- Results are cached best-effort by path, mtime and length in `$XDG_CACHE_HOME/rikka/material-seeds.toml`, 64 entries at most.
- The shell writes the result to `rikka.appearance/material-source`, and normal settings propagation re-themes everything.
