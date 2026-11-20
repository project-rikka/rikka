---
title: Windows executables and the portable Windows ZIP
description: rikka_application embeds manifest and version; windows_zip packs the five general apps and checks DLL imports.
about: bazel/windows
tags: [windows, release]
---

- `//bazel/windows:defs.bzl` `rikka_application(name, product, …)` wraps `rikka_binary`; every app under `apps/general` uses it. On Windows it links `<name>.res.o` (`windows_resource`: llvm-mingw `windres` over `resource.rc.in` and `application.manifest.in`) through `compile_data` + `-Clink-arg=$(location)`; Linux builds are unchanged.
- The resource holds the RT_MANIFEST id 1 (PerMonitorV2 + `true/pm`, UTF-8 code page, long paths, Common Controls 6, Windows 10/11 supportedOS, asInvoker) and VERSIONINFO (product Rikka, `Rikka <product>`, `<name>.exe`); executables carry no icon. The version comes from the string flag `--//bazel/windows:version` (default 0.1.0).
- `//apps/general:windows_zip` (manual, Windows only; `windows_zip` rule) runs `//xtask/windows/package:windows-package` into a flat archive: the five exes and the `NIX["windows_runtime"]/bin` DLLs at the root, `licenses/` = the runtime's licences, the icon font licences and Rikka's LICENSE and THIRD_PARTY notices (`licenses/rikka/`). There is no `share/`: Windows programs carry their UI and icon fonts compiled in (`rikka-embedded-fonts`), and `rikka-shipped` resolves `installed` paths beside the exe. Entries are sorted, case-insensitively unique, dated 1980-01-01, deflated.
- The packer fails when a program is not at the root or any packed .exe/.dll imports (`objdump -p`) a DLL that is neither at the root nor a Windows system DLL (`xtask/windows/package/src/system.rs`, api-ms-win-*/ext-ms-win-* api sets); add genuine system DLLs there.
- Build: `bazel run //xtask -- dist windows --version x.y.z --out <dir>` (= `--config=windows --config=release --//bazel/windows:version=…`) writes `rikka-<version>-windows-x86_64.zip`.
- Windows main.rs files carry `#![cfg_attr(windows, windows_subsystem = "windows")]`. Linux-only trees (system, apps/system, compositor, daemons, nix and Linux-API crates elsewhere) carry `target_compatible_with = ["@platforms//os:linux"]` so `--config=windows //...` skips them.
