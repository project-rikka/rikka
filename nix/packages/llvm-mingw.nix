{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  zlib,
  zstd,
  ncurses,
}:

let
  release = "20261020";
in
stdenv.mkDerivation {
  pname = "llvm-mingw";
  version = release;

  src = fetchurl {
    url = "https://github.com/mstorsjo/llvm-mingw/releases/download/${release}/llvm-mingw-${release}-ucrt-ubuntu-22.04-x86_64.tar.xz";
    hash = "sha256-VQZLYLmRoeHoCQuML4ToKyLP5ZA9EsFZ1lBkz16K4zU=";
  };

  nativeBuildInputs = [ autoPatchelfHook ];

  buildInputs = [
    stdenv.cc.cc.lib
    zlib
    zstd
    ncurses
  ];

  dontConfigure = true;
  dontBuild = true;
  dontStrip = true;

  autoPatchelfIgnoreMissingDeps = [ "libpython3*" ];

  installPhase = ''
    runHook preInstall
    rm -rf aarch64-w64-mingw32 armv7-w64-mingw32 i686-w64-mingw32
    rm -f bin/aarch64-* bin/armv7-* bin/arm64ec-*
    rm -f bin/lldb* bin/clangd bin/clang-tidy lib/liblldb*
    mkdir -p $out
    cp -a . $out/
    runHook postInstall
  '';

  meta = {
    description = "LLVM/Clang/LLD based mingw-w64 toolchain (UCRT) cross compiling for Windows";
    homepage = "https://github.com/mstorsjo/llvm-mingw";
    license = [
      lib.licenses.isc
      lib.licenses.asl20
      lib.licenses.llvm-exception
    ];
    platforms = [ "x86_64-linux" ];
  };
}
