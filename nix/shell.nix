{
  mkShell,
  callPackage,
  bazel_9,
  bazel-buildtools,
  rikka-fonts-conf,
}:

let
  env = callPackage ./env.nix { };
  bazel = callPackage ./bazel.nix { inherit env; };
in
mkShell {
  packages = [
    env.analyzerToolchain
    bazel_9
    bazel-buildtools
  ]
  ++ env.runtimeTools
  ++ env.bindingTools
  ++ env.websiteTools
  ++ [ env.geckoBuild ];

  env =
    env.shipped
    // bazel.environment
    // {
      FONTCONFIG_FILE = "${rikka-fonts-conf}";
      BAZEL_WRAPPER = "bazel/buildbuddy/wrapper";
    };

  shellHook = ''
    rikka_scratch="/tmp/rikka-dev-$(id -u)"
    mkdir -m 700 -p "$rikka_scratch" 2>/dev/null || true
    if [ -d "$rikka_scratch" ] && [ ! -L "$rikka_scratch" ] && [ -O "$rikka_scratch" ] && [ -w "$rikka_scratch" ]; then
      export TMPDIR="$rikka_scratch" TMP="$rikka_scratch" TEMP="$rikka_scratch" TEMPDIR="$rikka_scratch"
    else
      echo "rikka: $rikka_scratch is not a private writable directory; TMPDIR left unchanged" >&2
    fi
    unset rikka_scratch
  '';
}
