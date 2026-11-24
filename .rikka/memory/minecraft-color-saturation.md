---
title: Rikka Client Color Saturation mod (colour grade post pass)
description: mods/colorsaturation: hue, saturation, contrast, brightness as one affine colour matrix in a full-screen pass.
about: minecraft/src/main/java/sh/rikka/client/mods/colorsaturation
tags: [minecraft, rendering]
---

- `ColorSaturationMod` (`color_saturation`, VISUAL) ports Soar Client v7's ColorSaturationMod: settings `hue` (degrees, -180..180), `saturation`, `contrast` (0..2, 1 neutral), `brightness` (-1..1, 0 neutral).
- `ColorGrade` is the immutable uniform data: it folds the CSS Filter Effects hue-rotate and saturate matrices, contrast (pivot 0.5) and brightness into three row `vec4`s (a 3x4 affine matrix, w = offset). All of them keep gray on gray, so it equals applying them one after another without intermediate clamps. The mod rebuilds it only when a setting changes (`onChange`), and `neutral()` skips the pass.
- One pipeline (`ColorGradePipeline` over `render/post/ScreenPipeline`, fragment `assets/rikka/shaders/post/color_saturation/grade.fsh`, std140 block `ColorGrade`) draws into a `ScreenBuffer` and copies back to the main target (`ColorGradePass`), keeping source alpha. A neutral grade frees the buffers.
- `ColorSaturationHooks.levelFinished` runs from `GameRendererMixin` at `renderLevel` TAIL right after Motion Blur, so only the world (hand included) is graded, never the HUD or screens. A RuntimeException logs, frees the buffers and disables the mod.
