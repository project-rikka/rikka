{
  lib,
  addDriverRunpath,
  callPackage,
  pkgsCross,
  rust-bin,
  stdenv,
  mold-unwrapped,
  cmake,
  perl,
  pkg-config,
  rustPlatform,
  wayland-scanner,
  google-chrome,
  mesa,
  shader-slang,
  dbus,
  git,
  libsecret,
  nixfmt,
  shared-mime-info,
  xwayland,
  go,
  jdk25,
  python3,
  dotnet-sdk_10,
  nodejs,
  importNpmLock,
}:

let
  muslPkgs = pkgsCross.musl64;

  runtime = callPackage ./dist/runtime.nix { };
in
rec {
  inherit (runtime) dlopened;

  runpath = lib.concatStringsSep ":" [
    (lib.makeLibraryPath (dlopened ++ [ stdenv.cc.cc.lib ]))
    "${addDriverRunpath.driverLink}/lib"
    (lib.makeLibraryPath (lib.subtractLists dlopened buildInputs))
  ];

  mold = mold-unwrapped;

  muslTarget = muslPkgs.stdenv.hostPlatform.rust.rustcTarget;

  musl = muslPkgs.stdenv.cc;

  windowsTarget = "x86_64-pc-windows-gnullvm";

  windows = callPackage ./packages/llvm-mingw.nix { };

  windowsRuntime = callPackage ./dist/windows-runtime.nix {
    inherit (pkgsCross.ucrt64) spirv-tools;
  };

  rustToolchain =
    extensions:
    (rust-bin.fromRustupToolchainFile ../rust-toolchain.toml).override {
      extensions = (lib.importTOML ../rust-toolchain.toml).toolchain.components ++ extensions;
      targets = [
        muslTarget
        windowsTarget
      ];
    };

  toolchain = rustToolchain [ ];

  analyzerToolchain = rustToolchain [
    "rust-analyzer"
    "rust-src"
  ];

  nativeBuildInputs = [
    cmake
    perl
    pkg-config
    rustPlatform.bindgenHook
    wayland-scanner
  ];

  buildInputs = runtime.linked;

  slangc = lib.getExe' shader-slang "slangc";

  iconFonts = callPackage ./dist/icon-fonts.nix { };

  grammars = callPackage ./packages/grammars.nix { };

  discordKrisp = callPackage ./packages/discord-krisp.nix { };

  minecraftRuntime = callPackage ./dist/minecraft-runtime.nix { };

  shipped = {
    RIKKA_ICON_FONTS = "${iconFonts}/share/rikka/fonts";
    RIKKA_ICON_LISTINGS = "${iconFonts}/share/rikka/listings";
    RIKKA_GRAMMARS = "${grammars}/lib/rikka/grammars";
    RIKKA_MESA = "${mesa}";
    RIKKA_MINECRAFT = "${minecraftRuntime}/lib/rikka-minecraft";
  };

  chrome = lib.getExe' google-chrome "google-chrome-stable";

  runtimeTools = [
    dbus
    git
    libsecret
    nixfmt
    shared-mime-info
    xwayland
  ];

  bindings = {
    inherit go;
    jdk = jdk25;
    python = python3.withPackages (packages: [ packages.mypy ]);
    dotnet = dotnet-sdk_10;
  };

  bindingTools = builtins.attrValues bindings;

  website = {
    node = nodejs;
    modules = importNpmLock.buildNodeModules {
      package = lib.importJSON ../website/package.json;
      packageLock = lib.importJSON ../website/package-lock.json;
      inherit nodejs;
    };
  };

  websiteTools = [ website.node ];

  minecraftLibraries = minecraftRuntime.libraries;
}
