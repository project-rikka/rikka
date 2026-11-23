{ pkgs, release }:

let
  inherit (pkgs) lib;

  runtime = pkgs.callPackage ./runtime.nix { };

  fetched =
    artifact:
    if artifact ? url then
      pkgs.fetchurl { inherit (artifact) url hash; }
    else
      ./. + "/${artifact.file}";

  assets = lib.mapAttrs (
    id: artifact:
    pkgs.callPackage ./asset.nix {
      inherit id;
      src = fetched artifact;
    }
  ) release.assets;

  flake = {
    gecko = "${pkgs.callPackage ./gecko.nix { }}/lib/rikka-gecko";
    icon-fonts = "${pkgs.callPackage ./icon-fonts.nix { }}/share/rikka/fonts";
    mesa = "${pkgs.mesa}";
  };

  linked = name: flake.${name} or "${assets.${name}}";
in
lib.mapAttrs (
  id: entry:
  pkgs.callPackage ./component.nix {
    inherit id entry runtime;
    src = fetched entry;
    links = lib.mapAttrs (_: linked) entry.links;
  }
) release.components
