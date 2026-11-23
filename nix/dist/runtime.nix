{
  lib,
  addDriverRunpath,
  alsa-lib,
  ffmpeg_9-full,
  fontconfig,
  libdrm,
  libgbm,
  libinput,
  libopus,
  libx11,
  libxkbcommon,
  mesa,
  pam,
  pipewire,
  seatd,
  shader-slang,
  systemdLibs,
  vulkan-loader,
  wayland,
  zxing-cpp,
}:

let
  gbm = libgbm.overrideAttrs (previous: {
    mesonFlags = lib.filter (flag: !lib.hasPrefix "-Dgbm-backends-path=" flag) previous.mesonFlags ++ [
      (lib.mesonOption "gbm-backends-path" "${mesa}/lib/gbm:${addDriverRunpath.driverLink}/lib/gbm")
    ];
  });
in
{
  linked = [
    alsa-lib
    fontconfig
    libdrm
    gbm
    libinput
    libopus
    libx11
    libxkbcommon
    pam
    pipewire
    seatd
    systemdLibs
    wayland
    zxing-cpp
  ];

  dlopened = [
    ffmpeg_9-full.lib
    shader-slang
    vulkan-loader
    wayland
    libxkbcommon
  ];
}
