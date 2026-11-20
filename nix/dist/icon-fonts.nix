{
  lib,
  fetchurl,
  runCommand,
}:

let
  materialSymbols = "https://raw.githubusercontent.com/google/material-design-icons/737e3324305806514d7909874fa1818ae1808232";

  fontAwesome = "https://raw.githubusercontent.com/FortAwesome/Font-Awesome/7.3.1";

  shipped = {
    "MaterialSymbolsRounded.ttf" = {
      url = "${materialSymbols}/variablefont/MaterialSymbolsRounded%5BFILL,GRAD,opsz,wght%5D.ttf";
      sha256 = "95b24392bb49efd1bc3e92cff4e2452ad094461bab7c97e7d8723fab97e330ca";
    };
    "MaterialSymbols-LICENSE.txt" = {
      url = "${materialSymbols}/LICENSE";
      sha256 = "58d1e17ffe5109a7ae296caafcadfdbe6a7d176f0bc4ab01e12a689b0499d8bd";
    };
    "FontAwesomeBrands.otf" = {
      url = "${fontAwesome}/otfs/Font%20Awesome%207%20Brands-Regular-400.otf";
      sha256 = "0d1253642b9a55347cf6f020cedf47ac770379b76dd332db91ebe99fea7a06c2";
    };
    "FontAwesome-LICENSE.txt" = {
      url = "${fontAwesome}/LICENSE.txt";
      sha256 = "20c6f40715a567c97b80f6944beb8bb325835cab47ea7dcab89ee3b8e077eced";
    };
  };

  listings = {
    "MaterialSymbolsRounded.codepoints" = {
      url = "${materialSymbols}/variablefont/MaterialSymbolsRounded%5BFILL,GRAD,opsz,wght%5D.codepoints";
      sha256 = "225bd09137103cb7746bc93dc08d08764c9f0c3bd04f4b958d4a3c3c19432dd6";
    };
    "icons.json" = {
      url = "${fontAwesome}/metadata/icons.json";
      sha256 = "8b825be9cbe4a782a18343d912d24f7332cc2e297e62d0cad2203f8f4dd6bbca";
    };
  };

  placed =
    folder: files:
    lib.concatStrings (
      lib.mapAttrsToList (name: pin: ''
        install -Dm644 ${fetchurl ({ inherit name; } // pin)} $out/${folder}/${name}
      '') files
    );
in
runCommand "rikka-icon-fonts" { } ''
  ${placed "share/rikka/fonts" shipped}
  ${placed "share/rikka/listings" listings}
''
