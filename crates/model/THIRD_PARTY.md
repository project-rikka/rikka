# Third-party notices

## Blender

These crates port algorithms, data structures and behaviour from [Blender](https://projects.blender.org/blender/blender), Copyright (C) Blender Authors, licensed under the GNU General Public License version 2 or later (`GPL-2.0-or-later`). They are distributed as part of Rikka under the GNU General Public License version 3 or later (see `LICENSE` at the repository root):

- `rikka-mesh-edit`: the BMesh editable mesh kernel, its Euler operators, walkers, selection and custom-data interpolation.
- `rikka-mesh-bevel`: `bmesh_bevel.cc`, the Bevel operator and modifier and `BKE_curveprofile`.
- `rikka-mesh-boolean`: the exact and fast boolean solvers, the Intersect operators and the 2D constrained Delaunay triangulation.
- `rikka-mesh-reduce`: the BMesh removal and merge operators.
- `rikka-mesh-modifiers`: the mesh modifiers.
- `rikka-polymesh`: the mesh data layout, attributes and custom normals.
- `rikka-spatial`: the KD-tree and BVH queries.
