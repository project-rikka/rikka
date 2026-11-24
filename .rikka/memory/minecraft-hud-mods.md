---
title: Rikka Client panel HUD mods (minimap, keystrokes, target info, packs)
description: mods/minimap terrain atlas and drawing; Keystrokes, MouseStrokes, InventoryDisplay, PackDisplay, TargetInfo in mods/hud.
about: minecraft/src/main/java/sh/rikka/client/mods/minimap
tags: [minecraft, hud]
---

- Minimap, InventoryDisplay and TargetInfo are `OverlayHudMod`s: rikka-ui panels and text in `view`, game-native pixels (items, faces, map, entity models) in `extract(HudCanvas)` at the same box coordinates.
- Minimap: `TerrainCache` owns a 1024² RGBA `TerrainAtlas` (64×64 chunk slots addressed by chunk coordinate modulo 64, REPEAT/NEAREST sampler, so one quad UV = world block / 1024 draws any window). Each slot records its occupant chunk and a key (palette generation, colour mode, cave ceiling); `refresh` walks rings out from the player's chunk and rescans stale or dirty loaded chunks within a 3 ms budget per frame, vacating slots held by other chunks.
- Dirty marks come from `ChunkLoadEvent`/`BlockUpdateEvent`, also marking the south and east neighbours, whose height shading reads this chunk's edge. `ResourceReloadEvent` clears the palette and bumps the generation. A tick without a level detaches the cache from the old `ClientLevel`.
- `TerrainScanner` follows `MapItem`'s column rule (WORLD_SURFACE heightmap, skip `MapColor.NONE`, fluids replaced by their legacy block unless the top face is sturdy), blends water over its floor by depth, and shades by the height step to the north and west neighbours. Cave mode starts at a ceiling 2 blocks above the feet (4-block hysteresis), walks down up to 3 solid blocks to find air, else marks a wall. AUTO uses caves in ceilinged dimensions or with no sky light at the eyes.
- `TerrainPalette` BLOCKS colours average the first frame of the top-face (else unculled, else particle) sprite of the block model (`SpriteContentsAccessor.originalImage`), times its `BlockTintSource` in the world; fluids use the fluid model's still sprite and tint. MAP uses vanilla map colours.
- Drawing: `MapProjection` maps world ↔ box units (rotation = yaw + π when rotating, else 0) and is the `UvMap` of a feathered `GuiMesh.convex` in the `MapShape` outline; `MapMarkers` adds entity dots, player heads, the chevron and cardinal letters.
- Keystrokes reads movement/jump `KeyMapping.isDown` and mouse buttons from `MouseInputEvent` (a press since the last view still flashes); `animating()` looks only at keys it shows. CPS uses `mods/hud/ClickRates`, shared with CPS Display. MouseStrokes integrates `LookEvent` deltas into two `Smoothed` offsets returning to centre.
- TargetInfo follows `RikkaClient.targets()` (the local player while editing), fading/scaling in Java so the vanilla face matches the rikka panel; non-player portraits are a PiP entity model shown only past half visibility.
- PackDisplay lists the selected, non-required packs top first (else vanilla) with `pack.png` as an `Image.fromEncoded` and text widths measured once, refreshed on the first tick after enable and on `ResourceReloadEvent`; images are closed when replaced.
