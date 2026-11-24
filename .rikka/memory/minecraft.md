---
title: Minecraft Fabric mod build (Rikka Client)
description: minecraft/ holds the Rikka Client Fabric mod built by Bazel: lock, @minecraft repos, fabric_mod, run, mcsrc sources.
about: minecraft
tags: [minecraft, build]
---

- `minecraft/` is the Rikka Client Fabric mod (mod id and asset namespace `rikka`, Bazel target `//minecraft:rikka`, Java package `sh.rikka.client`, display name "Rikka Client", client-only, Mixin, no Fabric API). Sources in `src/main/java`, resources in `src/main/resources`; `fabric.mod.json` expands `${version}`, `${minecraft_version}`, `${loader_version}`, `${java_release}`. On disk it uses `config/rikka/` and `<gameDir>/rikka/` (music folder).
- Minecraft 26.x ships unobfuscated (Mojang names, LVT, parameters), so there is no remapping, no refmap and no Mixin annotation processor, as in Loom's non-remap mode. Mixins target Mojang names directly; MixinExtras is on the compile classpath.
- `minecraft/minecraft.lock.json` pins the client jar, its Linux libraries, JVM arguments, asset index, Fabric Loader with its libraries, and Vineflower, each with sha256. `bazel run //xtask -- minecraft [--version V] [--loader V]` rewrites it from Mojang's and Fabric's metadata, verifying upstream checksums.
- `//bazel/minecraft:extension.bzl` (included through `minecraft/minecraft.MODULE.bazel`) makes `@minecraft` (filegroups `client`, `libraries`, `fabric_loader`, `fabric_loader_development`, `vineflower`, plus `versions.bzl`) and `@minecraft_assets` (index + sha1-verified objects, fetched only when the client runs).
- `//bazel/minecraft:defs.bzl`: `fabric_mod` (javac `--release` from the lock, `-Xlint:all -Werror`, Loom-style manifest with `Fabric-Mapping-Namespace: official`), `minecraft_client` (KnotClient launcher), `minecraft_sources` (Vineflower).
- `//minecraft:client` and `//minecraft:sources` are `manual`, so `//...` builds only the mod and never fetches assets or decompiles.
- `bazel run //minecraft:client` starts the game with the mod in `minecraft/run/` (gitignored, per worktree), offline user, `log4j2.xml` from `bazel/minecraft`. It needs `NIX["minecraft_library_path"]` (glvnd, pulse, flite, X11 extras), which comes from `RIKKA_BAZEL_LOCAL`, not the RBE image. Shared libraries of the mod's Java deps (librikka.so.0) are prepended to its `LD_LIBRARY_PATH`; installed clients load the system librikka. The rule exports `SHIPPED_ENV` (`RunEnvironmentInfo`), so librikka finds the icon fonts in development runs.
- `fabric_mod(deps = [...rikka_java_library])` compiles against those jars and bundles their classes (minus `module-info.class`); the mod bundles `//sdk/bindings/java`.
- `bazel run //minecraft:sources` decompiles the client remotely and unpacks it into `minecraft/mcsrc/` (gitignored, read-only reference for writing mixins). Search it with an explicit path (`rg … minecraft/mcsrc`), since ignore rules hide it otherwise.
- `minecraft/` has no tests and gets none, by the person's decision; verify by building and by launching the client (mixin targets fail at startup, `defaultRequire: 1`).
- javac runs with `-Xlint:all,-classfile -Werror`; `classfile` is off because Gson, JOML and the client jar carry annotations whose classes are absent.
