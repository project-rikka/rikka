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

  coreTools = [
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
  ++ env.runtimeTools;

  toolEnv =
    name: paths:
    buildEnv {
      inherit name paths;
      pathsToLink = [
        "/bin"
        "/share"
      ];
    };

  tools = toolEnv "rikka-bazel-tools" coreTools;

  languageTools = toolEnv "rikka-bazel-language-tools" (
    coreTools ++ env.bindingTools ++ env.websiteTools
  );

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
    fontconfig_file = "${rikka-fonts-conf}";
    xdg_data_dirs = "${shared-mime-info}/share";
    ssl_cert_file = "${cacert}/etc/ssl/certs/ca-bundle.crt";
    discord_krisp = "${env.discordKrisp}/lib/discord_krisp";
    icon_fonts = env.shipped.RIKKA_ICON_FONTS;
    icon_listings = env.shipped.RIKKA_ICON_LISTINGS;
    grammars = env.shipped.RIKKA_GRAMMARS;
    mesa = env.shipped.RIKKA_MESA;
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
        '. + {
          pkg_config_path: $pkg_config_path,
          cflags: ($cflags | split(" ") | map(select(startswith("-frandom-seed=") | not)) | join(" ")),
          libclang_path: $libclang_path,
          bindgen_args: $bindgen_args
        }' \
        "$descriptionPath" > "$out"
    '';
  };

  languages = writeText "rikka-bazel-languages.json" (
    builtins.toJSON {
      language_path = "${languageTools}/bin";
      go = "${env.bindings.go}/share/go";
      java_home = "${env.bindings.jdk}/lib/openjdk";
      python = "${env.bindings.python}/bin/python3";
      dotnet = "${env.bindings.dotnet}/share/dotnet";
      node = "${env.website.node}/bin/node";
      website_modules = "${env.website.modules}/node_modules";
    }
  );

  local = writeText "rikka-bazel-local.json" (
    builtins.toJSON {
      minecraft_library_path = lib.makeLibraryPath env.minecraftLibraries;
      gecko = env.shipped.RIKKA_GECKO;
      rust_analyzer = "${env.analyzerToolchain}";
    }
  );

  rbeImage =
    paths: environment:
    dockerTools.streamLayeredImage {
      name = registry;
      contents = [
        paths
        dockerTools.fakeNss
      ];
      extraCommands = ''
        mkdir -m 1777 tmp
        mkdir -p usr/bin
        ln -s ../../bin/env usr/bin/env
      '';
      config.Labels."org.opencontainers.image.source" = "https://github.com/project-rikka/rikka";
      config.Env = [ "PATH=/bin" ] ++ environment;
      maxLayers = 120;
    };

  image = rbeImage tools [ "RIKKA_BAZEL_TOOLCHAIN=${toolchain}" ];

  languagesImage = rbeImage languageTools [
    "RIKKA_BAZEL_TOOLCHAIN=${toolchain}"
    "RIKKA_BAZEL_LANGUAGES=${languages}"
  ];
in
{
  inherit toolchain image languagesImage;

  environment = {
    RIKKA_BAZEL_TOOLCHAIN = "${toolchain}";
    RIKKA_BAZEL_LANGUAGES = "${languages}";
    RIKKA_BAZEL_LOCAL = "${local}";
    RIKKA_RBE_IMAGE = "${registry}:${image.imageTag}";
    RIKKA_RBE_LANGUAGES_IMAGE = "${registry}:${languagesImage.imageTag}";
  };
}
