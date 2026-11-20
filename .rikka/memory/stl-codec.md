---
title: STL codec (rikka-stl)
description: rikka-stl reads binary and ASCII STL in millimetres, with VisCAM and Magics colours, and writes both forms.
about: crates/model/rikka-stl
tags: [model, stl]
---

- `StlImporter` and `StlExporter::{binary, text}` (names "STL" and "STL Text") are registered in `rikka-model`.
- STL is read as millimetres, Z up (`STL_UNIT_SCALE`, `STL_COORDINATES`), so a model lands in metres; export writes millimetres back.
- Binary is chosen by exact file size before text, so binary files whose header starts with `solid` still read. Facet colours follow VisCAM (bit 15) or Magics (`COLOR=` header) palettes.
- Reading creates no material: a plain file is geometry only; facet or Magics header colours become linear vertex colours (unpainted facets 0.8 grey or the header colour).
- Reading welds corners by position, normal and colour; a facet normal that disagrees with its winding is replaced by the winding normal.
- Export writes world transforms, flips mirrored nodes, and writes colours only when the model has vertex colours or several material colours.
