---
title: FBX codec
description: rikka-fbx reads binary and text FBX from 6100 on, bakes animation, and writes FBX 7400.
about: crates/model/rikka-fbx
tags: [model, fbx]
---

- Ported from ufbx: `document.rs` (record tree), `euler.rs`, `read/`, `write/`.
- `FbxExporter::binary()` wins `.fbx`. `Registry::exporter_named("FBX Text")` picks text.
- `read/binary.rs` handles every version (wider headers from 7500, inflated arrays). `read/ascii.rs` reads the same tree from text. Names are `Name\0\x01Class` in binary and `Class::Name` in text.
- `read/graph.rs`: a connection's source is the child. Pre-7000 documents name objects instead of numbering them. `Definitions` templates back properties.
- `read/transform.rs` composes `T·Roff·Rp·Rpre·R·Rpost⁻¹·Rp⁻¹·Soff·Sp·S·Sp⁻¹` into one `Transform`. Geometric transforms, cameras and lights hang on child nodes, so cameras and lights face −Z.
- Geometry (`read/geometry.rs`, the one path for every version, inline 6100 meshes included): layers via mapping and reference modes, then the core `MeshAssembler` (one vertex per distinct corner keyed by control point, `polygon()` per face, so `Mesh::polygons` keeps quads/n-gons with `vertex_points` = FBX control-point numbers). Unused control points become loose vertices (in no polygon) so numbering and count survive; faces under 3 corners (loose edges) are dropped. Negative material indices mean no material (own submesh, `material: None`). Clusters and blend shapes address control points through `Shape::vertices_of_control`; `Transform` is the inverse bind; channel weight is the `DeformPercent` property, else the element.
- `read/curve.rs` solves Autodesk tangents (auto, TCB, weighted). `read/anim.rs` samples each stack at the union of key times through `bake::adaptive` with `Subdivision::Halves { rounds: 4 }` (subdividing where linear strays) and blends layers the way Autodesk does.
- Camera planes and light ranges are file units (scaled with the geometry); `OrthoZoom` is metres whatever the unit.
- Writing goes through `Composer` and produces 7400 with the Autodesk footer. Media is raw bytes in binary and base64 in text.
- `write/topology.rs`: `Topology::of(mesh)` is the polygons (control points compacted from `vertex_points`) or, without `polygons`, one triangle per three indices with one control point per vertex. `write/geometry.rs` writes the reference's layout from it: `PolygonVertexIndex`, `Edges` (first corner of each unique edge), normals `IndexToDirect` deduplicated (not with blend shapes, the reference's rule), tangents/binormals `Direct`, `Smoothing` `ByEdge` derived from corner normals (sharp where they differ or >2 faces), colours and UVs (`UVMap`, deduplicated per control point) `IndexToDirect`, materials `ByPolygon` (−1 for a submesh without material, layer omitted when none has one). `GeometryRecord::points` maps vertices to control points for clusters and shapes in `write/deform.rs`.
- Samples: `assets/knight.fbx` (CC0, 75 takes, tris + quads) and `assets/polygons.fbx` (the reference-made column: quads, octagons, two materials, skin, shape key) serve `tests/`. Writer checks use ufbx (in `reference/`) and the headless reference (binary only; the reference refuses text FBX).
