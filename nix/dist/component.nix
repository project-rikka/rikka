{
  lib,
  stdenv,
  addDriverRunpath,
  autoPatchelfHook,
  zstd,
  runtime,
  id,
  entry,
  src,
  links,
}:

stdenv.mkDerivation {
  pname = "rikka-${id}";
  inherit (entry) version;
  inherit src;

  nativeBuildInputs = [
    autoPatchelfHook
    zstd
  ];

  buildInputs = runtime.linked ++ [ stdenv.cc.cc.lib ];

  runtimeDependencies = runtime.dlopened;

  appendRunpaths = [ "${addDriverRunpath.driverLink}/lib" ];

  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall
    cp -r . "$out"
    ${lib.concatMapStrings (template: ''
      substituteInPlace "$out/${template}" --subst-var out
    '') entry.templates}
    if [ -d "$out/share/systemd/user" ]; then
      mkdir -p "$out/lib/systemd"
      mv "$out/share/systemd/user" "$out/lib/systemd/user"
    fi
    ${lib.concatStrings (
      lib.mapAttrsToList (into: target: ''
        mkdir -p "$(dirname "$out/${into}")"
        ln -s ${target} "$out/${into}"
      '') links
    )}
    runHook postInstall
  '';

  passthru = {
    inherit (entry)
      offer
      pam
      portal
      revision
      ;
    providedSessions = lib.optional (id == "base") "rikka";
  };

  meta = {
    description = entry.summary;
    homepage = "https://github.com/project-rikka/rikka";
    platforms = [ "x86_64-linux" ];
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
}
