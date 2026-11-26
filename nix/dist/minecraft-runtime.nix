{
  lib,
  runCommand,
  jdk25,
  alsa-lib,
  flite,
  libdecor,
  libglvnd,
  libpulseaudio,
  libx11,
  libxcursor,
  libxext,
  libxfixes,
  libxi,
  libxkbcommon,
  libxrandr,
  libxscrnsaver,
  systemdLibs,
  vulkan-loader,
  wayland,
}:

let
  libraries = [
    alsa-lib
    flite
    libdecor
    libglvnd
    libpulseaudio
    libx11
    libxcursor
    libxext
    libxfixes
    libxi
    libxkbcommon
    libxrandr
    libxscrnsaver
    systemdLibs
    vulkan-loader
    wayland
  ];
in
runCommand "rikka-minecraft-runtime" { passthru = { inherit libraries; }; } ''
  root="$out/lib/rikka-minecraft"
  mkdir -p "$root/lib"
  ln -s ${jdk25}/lib/openjdk "$root/java"
  for folder in ${lib.concatMapStringsSep " " (library: "${lib.getLib library}/lib") libraries}; do
    for file in "$folder"/*.so*; do
      if [ -f "$file" ]; then
        ln -sf "$file" "$root/lib/$(basename "$file")"
      fi
    done
  done
''
