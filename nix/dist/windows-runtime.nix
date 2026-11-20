{
  fetchurl,
  runCommand,
  p7zip,
  unzip,
  inter,
  jetbrains-mono,
  spirv-tools,
}:

let
  ffmpeg = fetchurl {
    url = "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-full_build-shared.7z";
    hash = "sha256-TSBgqLNKlAqkfXhRQgVbuSpjBTeB5V8qzkVG7dUZqPU=";
  };

  slang = fetchurl {
    url = "https://github.com/shader-slang/slang/releases/download/v2026.16/slang-2026.16-windows-x86_64.zip";
    hash = "sha256-f6HmnWhwbtGMxnknC8Oi5KP3QA2fe/OTVk+tOzvAPiU=";
  };

  interLicense = fetchurl {
    url = "https://raw.githubusercontent.com/rsms/inter/v${inter.version}/LICENSE.txt";
    hash = "sha256-JiSB6ERSGzJvXs0FPlm5jIstp4yO4b27boF0MF5Uk1o=";
  };

  jetbrainsMonoLicense = fetchurl {
    url = "https://raw.githubusercontent.com/JetBrains/JetBrainsMono/v${jetbrains-mono.version}/OFL.txt";
    hash = "sha256-MPDBNuPIjkItB5Gs2XI4hw+QVKlym8NM8v8NTtjKxK0=";
  };
in
runCommand "rikka-windows-runtime"
  {
    nativeBuildInputs = [
      p7zip
      unzip
    ];
  }
  ''
    mkdir -p $out/bin $out/share/rikka/fonts $out/share/licenses/{ffmpeg,slang,inter,jetbrains-mono}
    7z x -bd -o"$TMPDIR/ffmpeg" ${ffmpeg} > /dev/null
    ffmpeg_root=$(echo "$TMPDIR"/ffmpeg/ffmpeg-*)
    for dll in avcodec-63 avformat-63 avutil-61 swresample-7 swscale-10; do
      cp "$ffmpeg_root/bin/$dll.dll" $out/bin/
    done
    cp "$ffmpeg_root/LICENSE" $out/share/licenses/ffmpeg/LICENSE
    cp "$ffmpeg_root/README.txt" $out/share/licenses/ffmpeg/README.txt

    unzip -q ${slang} -d "$TMPDIR/slang"
    for dll in slang slang-compiler slang-glslang slang-glsl-module; do
      cp "$TMPDIR/slang/bin/$dll.dll" $out/bin/
    done
    cp "$TMPDIR/slang/LICENSE" $out/share/licenses/slang/LICENSE

    cp ${spirv-tools}/bin/libSPIRV-Tools-shared.dll $out/bin/SPIRV-Tools-shared.dll
    mkdir -p $out/share/licenses/spirv-tools
    cp ${spirv-tools.src}/LICENSE $out/share/licenses/spirv-tools/LICENSE

    cp ${inter}/share/fonts/truetype/InterVariable.ttf ${inter}/share/fonts/truetype/InterVariable-Italic.ttf $out/share/rikka/fonts/
    cp "${jetbrains-mono}/share/fonts/truetype/JetBrainsMono[wght].ttf" "${jetbrains-mono}/share/fonts/truetype/JetBrainsMono-Italic[wght].ttf" $out/share/rikka/fonts/
    cp ${interLicense} $out/share/licenses/inter/LICENSE.txt
    cp ${jetbrainsMonoLicense} $out/share/licenses/jetbrains-mono/OFL.txt
  ''
