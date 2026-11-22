---
title: Project website (website/)
description: Astro + TypeScript site on GitHub Pages at rikka.sh; Nix node_modules, theme colours and SDK examples from sources.
about: website
tags: [website, astro]
---

- `website/` is a static Astro 7 site (TypeScript, no UI framework). `//website` builds `dist` as a directory; `//website:types` (`astro check`) and `//website:format` (`prettier --check`) are tests; `bazel run //website:dev` serves it, `bazel run //website:prettier` formats it.
- Rules live in `//bazel/astro:defs.bzl`: `rikka_astro_site`, `rikka_astro_test`, `rikka_astro_run`. Build and tests copy `srcs` and `data` to a scratch tree at their workspace paths and link `NIX["website_modules"]` as `website/node_modules`; `run` links the same store path into the checkout (gitignored, in `.bazelignore`).
- npm dependencies come only from Nix: `nix/env.nix` `website.modules` is `importNpmLock.buildNodeModules` over `website/package.json` and `package-lock.json`, so a dependency change rebuilds the RBE image (`rbe-image.yml` watches both files). Update the lock with the shell's `npm install --package-lock-only`; there are no npm scripts.
- Workspace data enters by path, never copied: `plugins/theme` reads `rikka-light.toml`/`rikka.toml` into `virtual:rikka-theme` (palettes) and `virtual:rikka-theme.css` (`--<role>: light-dark(light, dark)`); `plugins/workspace.ts` serves `workspace:<path>` imports as text (the SDK counter examples). Each such file is `exports_files`-visible to `//website:__pkg__`.
- The mascot is the committed cut-out `website/src/assets/rikka.webp` (lossless, trimmed, 3200 px tall). `src/brand/mascot.ts` reads it with sharp from `process.cwd()` for `og.png` and the face icons; `FACE` is the crop matching the project-rikka GitHub org avatar, used by `favicon.ico`, `icon.png` and `apple-touch-icon.png`.
- The page is the hero and the SDK section, with no header; the theme toggle sits in the footer. The hero's install note links `site.existingNixos` (the existing-NixOS section of `docs/en/installation.md`). Page gradients live on `html` in `global.css` with a fixed grain layer on `body::before`.
- Theme choice: system by default, `data-theme` on `<html>` from `localStorage["rikka-theme"]`; choosing the system scheme clears it. CSP comes from Astro's `security.csp` as a meta tag; GitHub Pages sets no custom headers.
- Deployment is outside Bazel: `.github/workflows/website.yml` runs `npm ci` and `astro build` in `website/` with setup-node (Node 24) and Ubuntu's `fonts-inter` for `og.png`, then publishes `website/dist` with the Pages actions on pushes to `main` touching the site's inputs. The custom domain `rikka.sh` is set in the repository's Pages settings, not by a `CNAME` file.
