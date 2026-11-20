---
title: PSD/PSB format library (crates/graphics/rikka-psd)
description: rikka-psd: lossless PSD/PSB read/write, typed blocks, descriptors, engine data, pixels, ICC colour, style views.
about: crates/graphics/rikka-psd
tags: [psd, formats]
---

- Depends on rikka-types and rikka-simd (8/16-bit quantization in `pixels/quantize.rs`, exact round-half-away) plus flate2, moxcms (CMYK ICC) and thiserror. It and rikka-psd-render are public crates.
- `Document::read(&[u8])` / `to_bytes()` cover PSD and PSB, every colour mode and 1/8/16/32-bit; 16/32-bit layer info goes into `Lr16`/`Lr32` automatically.
- Round trip rule: every block is decoded into a typed value, and anything that fails to decode or has non-zero trailing bytes stays `Info::Raw`/`ResourceData::Raw`, so writing never loses data.
- `Document.layers` is a tree of `Node::{Layer, Group}`, children bottom-first; a `Group` keeps its folder record and its `</Layer group>` end record.
- Nodes are addressed by `NodePath` (group indices + index among siblings; `sibling`, `child`, `group`, `holds`, `after_removal`) and by pre-order position: `Document::{preorder, preorder_path, preorder_index, siblings, node, node_mut, remove_node, insert_node}` (`Error::NoPlace` when a path names no slot). Paint's `LayerId` is the pre-order position.
- Channels stay encoded (`Encoded::{Raw, Rle{rows, packed}, Zip, ZipPredicted}`); decode on demand with `Layer::decode_channel`, `Document::composite_plane` or `layer_rgba`/`composite_rgba`. `composite_rgba8` converts in row bands, so peak memory is the native planes plus the bytes.
- `FileHeader::{SIGNATURE, is_photoshop, pixel_count, composite_bytes}` serve signature sniffing and memory estimates before reading.
- `FileHeader::read` parses only the 26-byte header (used by `Document::read`). `has_real_composite` is false when the composite lacks colour planes or VersionInfo says it has no real merged data while layers exist.
- `Preamble::read(impl Read)` streams only header, colour mode data and resources (e.g. the stored thumbnail) without reading pixels.
- Untrusted sizes never drive allocation alone: compressed channels whose data cannot reach the decoded size fail with `CorruptChannel` before allocating, while short Raw data is zero-padded, so callers bound dimensions from the header first.
- The composite is matted against white when `merged_alpha`; `composite_rgba` unmattes and `set_composite_rgba` mattes. With no layers, writing `merged_alpha` emits an empty placeholder layer so the flag survives.
- `src/transform/` holds atomic whole-document geometry: `flip(Axis)`, `rotate_quarter(QuarterTurn)` (exact native-plane remaps), `resize_canvas(Rect, Color)` (crop is its enclosed case; only the Background layer is trimmed/painted, other pixels stay past the canvas) and `resample(Geometry, &ResampleOptions, &mut impl Resampler)` (`CpuResampler` is the reference; Paint supplies its own).
- Every transform plans new channels first (`Rebuild`), commits, then `Retarget` pushes one `Affine` through all typed metadata (paths as canvas fractions, text/placed/SoLd/vogk transforms, guides, slices, artboards, annotations, comps, gradient angles, resolution on axis swaps, effect sizes via `Effects::scale_sizes`) and drops thumbnails. `Layer::put_channel` places an already encoded channel.
- Descriptors keep their OSTypes (`Value`), ids remember 4-char vs text form (`Id`), and `DescriptorBlock` keeps each key's header (`lfx2` u32, `clrL` u16, `SoLd` key+version…).
- Tagged-block padding follows the format's convention: most keys count padding to 2, a listed set to 4, and `Txt2`/`cinf`/`extn`/`CAI `/`OCIO` pad outside the length; PSB uses 8-byte lengths for the psd-tools key list.
- `style/` holds editable views built from descriptors and written back over the source descriptor so unknown keys survive: `Effects`, `FillContent`, `Gradient`, `Contour`, `VectorStroke`, `TextLayer` (EngineData), adjustment views, and `Layer::{kind, effects, set_effects, fill, smart_object, …}`.
- `ColorConverter` converts native planes to RGB: CMYK through an embedded ICC profile (naive formula otherwise; an embedded profile that is unreadable, not CMYK or untransformable is reported by `profile_issue()` as a `ProfileIssue`), Lab with a D50→sRGB matrix, 32-bit stays linear (`RgbaImage.linear`).
- Reference clones for checking against real files: `reference/ag-psd`, `reference/psd-tools`, `reference/gimp` (git-ignored).
