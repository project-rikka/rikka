---
title: Rikka Client Motion Blur mod (velocity, frame blending, accumulation)
description: mods/motionblur: motion blur on own RenderPipelines; hook points, passes, frame history, uniforms.
about: minecraft/src/main/java/sh/rikka/client/mods/motionblur
tags: [minecraft, rendering]
---

- `MotionBlurMod` (`motion_blur`, VISUAL) offers VELOCITY, FRAME_BLENDING, HYBRID, ACCUMULATION_MAX and ACCUMULATION_MIX, configured only through its mod settings.
- Passes are own `RenderPipeline`s (`MotionBlurPipelines` over `render/post/ScreenPipeline`: vertex `core/screenquad`, shared `MAIN_SAMPLER`, fragments in `assets/rikka/shaders/post/motion_blur/`, variants by shader defines) drawn by `render/post/ScreenPass`. Velocity and accumulation draw into their own color-only RGBA8 target (`render/post/ScreenBuffer`) and copy it to the main target; frame blending reads only history frames, so it draws straight into the main target. Uniform blocks come from `render/post/UniformStream` (a vanilla `DynamicGpuDataStorageMapped`, `endFrame` once per frame in `finish`). `render/post` is the shared kit for full-screen post passes (Color Saturation uses it too).
- Hooks (`MotionBlurHooks`, render thread): `GameRendererMixin` calls `levelStarting` just before `LevelRenderer.render` (camera state + the bobbed `projectionMatrix` local), `levelDrawn` right after it, `levelFinished` at `renderLevel` TAIL (after the hand). `LevelRendererMixin` closes the "Main" pass after opaque terrain, runs the terrain pass, and opens one pass for solid features that classic transparency continues through `@WrapMethod` (closed at once with improved transparency). `WindowMixin` invalidates the cached refresh rate on display/fullscreen SDL events.
- Velocity: first person draws TRANSLATION (depth motion minus camera rotation) before entities and ROTATION after the level; third person or riding draws FULL before entities only. Refresh rate scaling multiplies strength and samples by max(1, fps/Hz); HYBRID uses a ≤1-frame filler strength.
- Frame blending: `FrameHistory` is a ring of at most 24 frames sized to the exposure (strength / min(fps, Hz)) plus 2; capacity grows at once and shrinks only after 120 frames of lower demand. When the window needs more frames the newest slot is overwritten until a minimum interval passes, so 24 slots always span it. `ExposureWeights` box-weights frames by time overlap.
- `CameraMotion` treats a frame gap ≥ 1 s, a level change or a > 16-block jump as a cut (no previous frame, history and accumulation restart); panoramic screenshots skip the effect.
- Any RuntimeException in a stage logs, releases GPU resources and disables the mod; `onDisable` frees every target at once, and switching algorithms frees unused ones on the next frame.
