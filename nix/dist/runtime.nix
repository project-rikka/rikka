{
  alsa-lib,
  ffmpeg_9-full,
  fontconfig,
  libdrm,
  libgbm,
  libinput,
  libopus,
  libx11,
  libxkbcommon,
  pam,
  pipewire,
  seatd,
  shader-slang,
  systemdLibs,
  vulkan-loader,
  wayland,
  zxing-cpp,
}:

{
  linked = [
    alsa-lib
    fontconfig
    libdrm
    libgbm
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
