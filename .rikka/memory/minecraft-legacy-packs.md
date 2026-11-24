---
title: Rikka Client legacy resource pack conversion
description: sh.rikka.client.resource(.legacy): in-memory upgrade of pre-26.3 resource packs, data files, upgrade steps, harness.
about: minecraft/src/main/java/sh/rikka/client/resource
tags: [minecraft, resource packs]
---

- Every client resource pack (resourcepacks/ and server-sent) whose pack format is below the game's is converted in memory; the pack file is never written. `FolderRepositorySourceMixin` (only `CLIENT_RESOURCES`) and `DownloadedPackSourceMixin` swap its `Pack.ResourcesSupplier` through `LegacyPacks.supplier(path, original)`.
- `LegacyPackSupplier` serves a rewritten `pack.mcmeta` (`PackConverter.metadata`: `min_format`/`max_format` = current, overlays dropped) so the pack shows as compatible; `LegacyPackResources` opens the archive lazily, reports `PackConverter.namespaces` (normalized, without converting) and converts on first resource access (on reload workers), closing on `close()`.
- `sh.rikka.client.resource.legacy` is Minecraft-free (Gson, slf4j only) and exposes only `PackConverter`, `PackDescriptor`, `PackArchive`, `Assets`/`Asset`, `Raster`, `ImageDecoder` and `Vanilla`; everything else is package-private. `PackConverter.convert(archive, descriptor, vanilla, decoder)` loads `assets/` plus the source's selected overlays and runs `Upgrades.STEPS`, each gated by `sourceFormat < before` (`PackFormats` constants). The adapter injects `NativeImageDecoder` and `VanillaReader` (the running game's vanilla pack, used as layout reference).
- Stored assets keep no bytes or rasters (read from the archive on demand); generated images are lazy `Asset.image(RasterSource)`, encoded once by the own `PngEncoder` and then release their raster and source. JSON models, blockstates, items and atlases are parsed once into `Documents`, so renames rewrite references (`Renaming`), and are encoded compactly when read.
- Data lives in `src/main/resources/sh/rikka/client/resource/legacy/`: `renames.json` (texture/model renames per data step), `sheets.json` (sheet slicing: particles, paintings, effects, GUI sprites, map icons, moon), `lang.json` (1.13 language keys). They were generated from vanilla jar diffs by the scratch tools in `.rikka/analysis/` (Gen*.java), which is not part of the build.
- Adding a game version: add its format to `PackFormats`, a `DataRenames` step and any structural upgrade to `Upgrades.STEPS`, and regenerate the data against the new vanilla jar.
- GUI handling: `ContainerLayouts` moves pre-1.20.2 container art to the current layouts, `GuiStyle`/`GuiPalette`/`GuiComposer` learn a pack's container palette and slot art, and `GuiSynthesis` composes containers and sprites the pack lacks only when its inventory differs from vanilla.
- An upgrade step that throws is skipped with a warning and the rest still run; a failure outside the steps serves the pack empty with an error log.
