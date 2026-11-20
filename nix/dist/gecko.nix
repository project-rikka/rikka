{
  lib,
  stdenv,
  fetchurl,
  addDriverRunpath,
  autoPatchelfHook,
  patchelfUnstable,
  wrapGAppsHook3,
  zstd,
  adwaita-icon-theme,
  alsa-lib,
  atk,
  cairo,
  dbus,
  ffmpeg_9-full,
  fontconfig,
  freetype,
  gdk-pixbuf,
  glib,
  gtk3,
  libdrm,
  libgbm,
  libglvnd,
  libice,
  libkrb5,
  libpulseaudio,
  libsecret,
  libsm,
  libva,
  libx11,
  libxcb,
  libxcomposite,
  libxcursor,
  libxdamage,
  libxext,
  libxfixes,
  libxi,
  libxrandr,
  libxrender,
  libxscrnsaver,
  pango,
  pipewire,
  systemdLibs,
  vulkan-loader,
}:

let
  pin = lib.importJSON ./gecko.json;
in
stdenv.mkDerivation {
  pname = "rikka-gecko";
  inherit (pin) version;

  src = fetchurl {
    name = "rikka-gecko-${pin.version}-x86_64-linux.tar.zst";
    inherit (pin) url hash;
  };

  sourceRoot = "rikka-gecko";

  nativeBuildInputs = [
    autoPatchelfHook
    patchelfUnstable
    wrapGAppsHook3
    zstd
  ];

  buildInputs = [
    adwaita-icon-theme
    alsa-lib
    atk
    cairo
    dbus
    fontconfig
    freetype
    gdk-pixbuf
    glib
    gtk3
    libx11
    libxcb
    libxcomposite
    libxcursor
    libxdamage
    libxext
    libxfixes
    libxi
    libxrandr
    libxrender
    pango
    stdenv.cc.cc.lib
  ];

  appendRunpaths = [
    "${addDriverRunpath.driverLink}/lib"
  ]
  ++ map (package: "${lib.getLib package}/lib") [
    ffmpeg_9-full
    libdrm
    libgbm
    libglvnd
    libice
    libkrb5
    libpulseaudio
    libsecret
    libsm
    libva
    libxscrnsaver
    pipewire
    systemdLibs
    vulkan-loader
  ];

  patchelfFlags = [ "--no-clobber-old-sections" ];

  dontWrapGApps = true;

  installPhase = ''
    runHook preInstall
    mkdir -p "$out/lib/rikka-gecko"
    cp -r . "$out/lib/rikka-gecko"
    runHook postInstall
  '';

  postFixup = ''
    wrapGApp "$out/lib/rikka-gecko/rikka-gecko"
  '';

  meta = {
    description = "Gecko as an embeddable engine for Rikka";
    homepage = "https://github.com/project-rikka/rikka/tree/main/engine/gecko";
    license = lib.licenses.mpl20;
    platforms = [ "x86_64-linux" ];
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
}
