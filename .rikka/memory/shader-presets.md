---
title: Shader presets (rikka-shader-presets, rikka-shader-presets-gpu)
description: Preset graph registry (classic, games, styles), the shared import parameters, prebuilt template artifacts.
about: crates/graphics/rikka-shader-presets
tags: [shaders, materials]
---

## Registry and import convention

- `rikka-shader-presets` is public and renderer-free: `Preset { key, title, summary, family, build }`, `presets()` concatenates `SETS` in `src/lib.rs` (one line per module: `classic`, `games`, `styles`), `preset(key)`. Each `build(&Library)` writes a Material graph with `Builder` from `Library::standard()` nodes; `Builder::frame(title, ..)` groups a section into a titled frame.
- `import.rs` is the convention every preset and rikka-model-gpu share: parameter names (`base_color` rgba `Role::Albedo`, `base_texture`, `shade_color`, `emission_color`/`emission_texture` with emission roles, `normal_texture`, `normal_scale`, `outline_width`, `outline_color`; PMX `toon_texture`, `sphere_texture`, `sphere_mode` none|add|multiply, `specular_color`, `specular_power`, `ambient_color`; `porosity`, `skin_reject`, the generated `see_through.amount/reach`), one helper per name declaring it on a `Builder`, `Class` (cloth, skin, face, hair, eye, mouth, lash, blush, stocking) whose names are parameter preset names, and `classed(..)`. A preset declares the subset it uses; importers set every name and skip what the schema lacks.
- Games and styles name the shared parameters through the `import` constants but declare them with their own groups, labels, defaults and soft ranges; games declare `emission_color` linear (`srgb: false`) where `import.rs` and styles use sRGB.
- `shade_color` is an absolute shadow-side colour (MToon semantics). `outline_width` is metres in world mode; `mmd` and MToon screen mode read it as a share of half the screen height (`npr.outline_width` mode `screen`).
- `tests/parameters.rs`: every preset declares `base_color` and `base_texture`, shared names have the convention's kind and role, every declared parameter feeds a node.

## Sets

- Classic (`src/classic/`): `mmd`, `mtoon`, and `anime` (`Family::Stylized`) from one `character(library, Style)` builder.
- Games (`src/games/`): `genshin_body|face`, `starrail_body|hair|face`, `zenless_body|hair|face`. `games/mod.rs` holds the import declarations and shared node groups (`Base Color`, `Normal Map`, `Optional Map`, `Incoming Light`, `Flat Ambient`, `Face Shadow`). Missing maps are detected with `texture.bound`: `Optional Map` substitutes the recipe's neutral texel (Genshin/HSR lightmap (0, 0.5, 0, 0), ZZZ `_N` (0.5, 0.5, 0.5, 1), `_M` (1, 0, 0, 1), `_A` (1, 0.5, 0, 1)), ramps fall back to shade colours, faces to `npr.face_procedural`. Data maps must be uploaded unpremultiplied. ZZZ's UV-channel outline normals arrive as `MeshData::outline_normals`.
- Styles (`src/styles/`: `mod.rs` declarations and blocks, `kit.rs`, `blocks.rs`, `realistic.rs`, `endfield.rs`, `hybrid.rs`, `toon.rs`, `looks.rs`): `pbr`, `pbr_cloth|glass|skin|hair|eye` rebuild `rikka/standard|skin|hair|eye` from closures (pixel-identical unlooked; `realistic::openpbr` is shared with `hybrid`); `endfield_body|skin|hair|face` (two-band cel diffuse + physically based highlights from one closure); `hybrid` (Mix of OpenPBR and stylized Lighting by `stylization`); looks `halftone`, `hatching`, `sketch`, `painterly`, `gooch`, `toon_basic`, `flat`, `glow`. Lit NPR parts scale like physical diffuse (albedo × tone × radiance × falloff ÷ π).

## Prebuilt templates

- `rikka-shader-presets-gpu` (public) embeds prebuilt templates so shipped presets need no libslang: `template(key) -> Option<MaterialTemplate>` (one cached template per key, so pipelines are shared) and `keys()`. Its `programs.bzl` rule `preset_programs(presets = [...], library = renderer :lib)` runs `presets-tool template --key K` once per key (one remote action each; the same path as `ShaderCompiler::template`) and a `listing` action writing `src/presets.rs`.
- A preset added to `SETS` must also be listed in that BUILD's `presets`; `tests/prebuilt.rs` checks both lists agree, that prebuilt templates report `Built` once drawn, and that run-time compiled mmd/mtoon drafts draw within 2 of the prebuilt ones and their optimized builds byte-identical. The template name is the preset key, which `Overrides` scopes use.
- `MaterialTemplate::prebuilt(name, &TemplateArtifact)` makes a non-reloadable template from any artifact; `TemplateArtifact` is re-exported by rikka-renderer-3d.
- GPU tests: `rikka-shader-presets-gpu/tests/` (PMX conformance, coloured shadows, emission bloom, anime face/hair shadow, see-through eyes); run-time compiles of games and styles in `rikka-shader-compile/tests/{games,presets,styles}.rs` (target `limit = 240`; they draw drafts, while a background `-O2` build of a template takes 20–80 s and 9–19 GB on this machine). Tests that need the optimized build wait with `rikka_test_renderer_3d::built`.

## Engine facts

- A closure port wired from a parameter forces its `[Feature]` bit on, so graph presets always run every exposed lobe.
- Several heavy closures may share a graph: each OpenPBR-model closure costs about one closure's compile (2 or 4 Principled/skin closures build in the same ~16 s as one; `rikka-shader-compile/tests/closures.rs`). Open shading cost still grows with each closure's own light loop.
- A subsurface closure reaching the output through Mix/Add is scattered in screen space; its forward result (what Shader-to-RGB reads) then lacks the subsurface diffuse. Even mixes of identical skin closures scatter exactly like one.
- Eyes over hair use the draw layers plus the `see_through.amount` trait; the graph cannot drive a view-dependent see-through alpha.
