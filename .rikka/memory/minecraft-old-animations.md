---
title: Rikka Client Old Animations mod (1.7 visuals)
description: mods/animation: one visual-only Mod, its mixin hooks, and the exact-matrix method that maps 1.7 item chains onto modern models.
about: minecraft/src/main/java/sh/rikka/client/mods/animation
tags: [minecraft, visuals]
---

- `OldAnimationsMod` (id `old_animations`, Category VISUAL) owns every setting and `active()`; mixins only call `AnimationHooks`, which returns vanilla behaviour when the mod is off. Everything is client-side drawing: no packets, no gameplay change (blocking blocks no damage, `punch_blocks` swings locally and spawns hit particles only).
- Sword blocking is fake use state: `SwordBlocking` is engaged while right click is held with a `blocking_items` item and nothing consumed the click. `MinecraftMixin` wraps `interact`/`useItemOn`/`useItem` in `startUseItem` and spends the click on any `Success`. `AvatarRendererMixin` makes the local player's main-hand arm pose BLOCK; real server-driven BLOCK on non-shield items gets the same pose.
- First person: `FirstPersonHandsAndItemsRendererMixin` cancels vanilla `submitArmWithItem` for the right main arm (plain, eat/drink, bow, block); `FirstPersonPose.chain` rebuilds the 1.7.10 `renderItemInFirstPerson` chain (bob, base translate, swing rotations, 0.4 scale, block/bow insert, eat jiggle first). Left-handed mains only get the mirrored block pose.
- Third person: `ItemInHandLayerMixin` wraps `ItemStackRenderState.submit` and pre-multiplies a correction from `ThirdPersonPose`; `HumanoidModelMixin` poses the blocking arm (`ThirdPersonBlock` V1_7/V1_8/MODERN) and the 1.7 sneak pose. Shields, spears, babies and non-avatars stay vanilla.
- Method: the 1.7 sprite chain (`ItemInHand` + `SpritePlacement.item()`) is fitted to the model's actual display transform (`DisplayedItem`, read through `ItemStackRenderStateAccessor`/`LayerRenderStateAccessor`) as `target * (display * spriteToBlock)^-1`. The sprite-to-block map is a rotation when the target has positive determinant and a reflection otherwise, so the final matrix is always proper. `ItemFamily` recognises vanilla generated/handheld/rod/bow display transforms; anything else (blocks, custom models, specials) keeps vanilla placement.
- Other hooks: `CameraMixin` re-derives `eyeHeight` after `Camera.tick` (instant drop of `sneak_depth`, optional 0.6/tick rise, driven by `isCrouching`, so the server pose echo cannot bounce it); `HudMixin` forces heart `blink` false; `EquipmentLayerRendererMixin` swaps `NO_OVERLAY` for the entity's hurt overlay on worn equipment.
- Not included on purpose: attack-while-using, blocking slowdown, 1.7 hitbox/eye height, enchantment glint and 2D dropped items.
