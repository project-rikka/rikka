{
  stdenv,
  autoPatchelfHook,
  zstd,
  id,
  src,
}:

stdenv.mkDerivation {
  pname = "rikka-${id}";
  version = "0";
  inherit src;

  nativeBuildInputs = [
    autoPatchelfHook
    zstd
  ];

  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall
    cp -r . "$out"
    runHook postInstall
  '';
}
