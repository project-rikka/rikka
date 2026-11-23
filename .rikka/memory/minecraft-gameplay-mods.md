---
title: Rikka Client camera, mechanic and visual mods
description: mods/camera (FreeLook, TapLook, Zoom), mods/mechanic (SnapTap, HitDelayFix, NoJumpDelay), mods/visual and their hooks.
about: minecraft/src/main/java/sh/rikka/client/mods
tags: [minecraft, mods]
---

- Hold-style mods (FreeLook Left Alt, TapLook V, Zoom C) pass `DefaultBind.hold(key)` to the `Mod` constructor, so the framework's own `bind`/`bind_mode` controls make "enabled" mean "active"; switching the mode to Toggle gives the toggle variant. Mods without a default bind use the three-argument constructor.
- Each mod keeps a `static volatile active` (or boolean) set in `onEnable`/`onDisable`; mixins call only the package's public `*Hooks` class (`CameraHooks`, `MechanicHooks`, `VisualHooks`), which returns the vanilla value when nothing is active.
- `camera/`: `CameraHooks` overrides `Camera.alignWithEntity`'s `getViewYRot/XRot` (FreeLook, local player only), `Camera.calculateFov` (Zoom) and `Options.getCameraType` (TapLook wins over FreeLook; FreeLook only swings out from first person). The real camera option is never written, so nothing needs restoring.
- FreeLook never turns the body: `MouseHandler.turnPlayer` wraps `LocalPlayer.turn`, and `FreeLookMod` keeps its own yaw/pitch (seeded lazily from the player, optional yaw limit around that anchor). The same wrap multiplies the turn by `Zoom.sensitivity()`.
- `Zoom` is a log-magnification, critically damped spring (closed form, frame-rate independent) kept outside the mod so it keeps easing after the mod is disabled; fov is `2·atan(tan(fov/2)/magnification)`. Scroll while zoomed cancels `MouseScrollEvent` and moves the target.
- `mechanic/`: SnapTap records press order in `KeyMapping.setDown` and rewrites the four movement reads in `KeyboardInput.tick` through one `WrapOperation` on `KeyMapping.isDown` (last pressed wins, older key resumes on release); HitDelayFix turns `MultiPlayerGameMode.hasMissTime()` false inside `Minecraft.startAttack`; NoJumpDelay zeroes `LivingEntity.noJumpDelay` for the `LocalPlayer` in `aiStep`.
- `visual/`: Fullbright replaces the `Math.max` brightness in `LightmapRenderStateExtractor.extract`; ForceMainHand changes `Avatar.getMainArm` for `RemotePlayer`s only. BloodParticle and Particles react to `HitEvent` (server-confirmed own melee hit); Particles' NATURAL trigger multiplies the server's `ClientboundAnimatePacket` critical/enchanted-hit signals, ALWAYS adds bursts to each own hit. ProjectileTrail spaces particles along the last tick's path within 32 blocks (the particle engine's own cut-off).
- Enum setting labels live under `rikka-client.mod.<id>.setting.<setting>.<constant>`; `Viewpoint` labels are shared under `rikka-client.setting.perspective.*`.
