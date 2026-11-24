{
  lib,
  stdenv,
  buildEnv,
  dockerTools,
  writeText,
  jq,
  bashNonInteractive,
  coreutils,
  diffutils,
  findutils,
  gawk,
  gnugrep,
  gnumake,
  gnused,
  gnutar,
  gzip,
  bzip2,
  xz,
  zstd,
  rikka-fonts-conf,
  shared-mime-info,
  cacert,
  env,
}:

let
  registry = "ghcr.io/project-rikka/rikka-rbe";

  tools = buildEnv {
    name = "rikka-bazel-tools";
    paths = [
      bashNonInteractive
      coreutils
      diffutils
      findutils
      gawk
      gnugrep
      gnumake
      gnused
      gnutar
      gzip
      bzip2
      xz
      zstd
      stdenv.cc
      env.toolchain
    ]
    ++ env.nativeBuildInputs
    ++ env.runtimeTools
    ++ env.bindingTools
    ++ env.websiteTools;
    pathsToLink = [
      "/bin"
      "/share"
    ];
  };

  compiler =
    cc:
    let
      bintools = cc.bintools;
    in
    {
      cc = "${cc}/bin/${cc.targetPrefix}gcc";
      cpp = "${cc}/bin/${cc.targetPrefix}cpp";
      ar = "${bintools}/bin/${cc.targetPrefix}ar";
      nm = "${bintools}/bin/${cc.targetPrefix}nm";
      objcopy = "${bintools}/bin/${cc.targetPrefix}objcopy";
      objdump = "${bintools}/bin/${cc.targetPrefix}objdump";
      strip = "${bintools}/bin/${cc.targetPrefix}strip";
      ld = "${bintools}/bin/${cc.targetPrefix}ld";
    };

  description = {
    bash = lib.getExe bashNonInteractive;
    path = "${tools}/bin";
    rust = "${env.toolchain}";
    mold = "${env.mold}/bin";
    gnu = compiler stdenv.cc;
    musl = compiler env.musl // {
      target = env.muslTarget;
    };
    windows =
      let
        prefix = "${env.windows}/bin/x86_64-w64-mingw32-";
      in
      {
        target = env.windowsTarget;
        cc = "${prefix}clang";
        cpp = "${prefix}clang";
        ar = "${prefix}ar";
        nm = "${prefix}nm";
        objcopy = "${prefix}objcopy";
        objdump = "${prefix}objdump";
        strip = "${prefix}strip";
        ld = "${prefix}ld";
        dlltool = "${prefix}dlltool";
        windres = "${prefix}windres";
        sysroot = "${env.windows}/x86_64-w64-mingw32";
      };
    windows_runtime = "${env.windowsRuntime}";
    runpath = env.runpath;
    library_path = lib.makeLibraryPath env.buildInputs;
    slangc = env.slangc;
    go = "${env.bindings.go}/share/go";
    java_home = "${env.bindings.jdk}/lib/openjdk";
    python = "${env.bindings.python}/bin/python3";
    dotnet = "${env.bindings.dotnet}/share/dotnet";
    node = "${env.website.node}/bin/node";
    website_modules = "${env.website.modules}/node_modules";
    fontconfig_file = "${rikka-fonts-conf}";
    xdg_data_dirs = "${shared-mime-info}/share";
    ssl_cert_file = "${cacert}/etc/ssl/certs/ca-bundle.crt";
    discord_krisp = "${env.discordKrisp}/lib/discord_krisp";
    icon_fonts = env.shipped.RIKKA_ICON_FONTS;
    icon_listings = env.shipped.RIKKA_ICON_LISTINGS;
    grammars = env.shipped.RIKKA_GRAMMARS;
    gecko = builtins.unsafeDiscardStringContext env.shipped.RIKKA_GECKO;
    mesa = builtins.unsafeDiscardStringContext env.shipped.RIKKA_MESA;
  };

  toolchain = stdenv.mkDerivation {
    name = "rikka-bazel-toolchain.json";
    dontUnpack = true;
    dontConfigure = true;
    dontBuild = true;
    dontFixup = true;
    nativeBuildInputs = env.nativeBuildInputs ++ [ jq ];
    inherit (env) buildInputs;
    passAsFile = [ "description" ];
    description = builtins.toJSON description;
    installPhase = ''
      jq \
        --arg pkg_config_path "$PKG_CONFIG_PATH" \
        --arg cflags "$NIX_CFLAGS_COMPILE" \
        --arg libclang_path "$LIBCLANG_PATH" \
        --arg bindgen_args "$BINDGEN_EXTRA_CLANG_ARGS" \
        '. + {pkg_config_path: $pkg_config_path, cflags: $cflags, libclang_path: $libclang_path, bindgen_args: $bindgen_args}' \
        "$descriptionPath" > "$out"
    '';
  };

  local = writeText "rikka-bazel-local.json" (
    builtins.toJSON {
      minecraft_library_path = lib.makeLibraryPath env.minecraftLibraries;
    }
  );

  image = dockerTools.streamLayeredImage {
    name = registry;
    contents = [
      tools
      dockerTools.fakeNss
    ];
    extraCommands = ''
      mkdir -m 1777 tmp
      mkdir -p usr/bin
      ln -s ../../bin/env usr/bin/env
    '';
    config.Labels."org.opencontainers.image.source" = "https://github.com/project-rikka/rikka";
    config.Env = [
      "PATH=/bin"
      "RIKKA_BAZEL_TOOLCHAIN=${toolchain}"
    ];
    maxLayers = 120;
  };
in
{
  inherit toolchain image;

  environment = {
    RIKKA_BAZEL_TOOLCHAIN = "${toolchain}";
    RIKKA_BAZEL_LOCAL = "${local}";
    RIKKA_RBE_IMAGE = "${registry}:${image.imageTag}";
  };
}
