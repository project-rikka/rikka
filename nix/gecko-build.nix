{
  buildFHSEnv,
  writeShellScript,
  bashNonInteractive,
  coreutils,
  diffutils,
  findutils,
  gawk,
  git,
  gnugrep,
  gnumake,
  gnused,
  gnutar,
  gzip,
  perl,
  python3,
  rustup,
  stdenv,
  unzip,
  which,
  xz,
  zlib,
  zstd,
}:

buildFHSEnv {
  name = "rikka-gecko-build";

  targetPkgs = _: [
    bashNonInteractive
    coreutils
    diffutils
    findutils
    gawk
    git
    gnugrep
    gnumake
    gnused
    gnutar
    gzip
    perl
    python3
    rustup
    stdenv.cc.cc.lib
    unzip
    which
    xz
    zlib
    zstd
  ];

  runScript = writeShellScript "rikka-gecko-build-run" ''exec "$@"'';
}
