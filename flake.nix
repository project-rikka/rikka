{
  description = "Rikka -- a Wayland session with a modern UI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      system = "x86_64-linux";

      pkgs = import nixpkgs {
        inherit system;
        overlays = [ self.overlays.default ];
      };
    in
    {
      overlays.default = nixpkgs.lib.composeExtensions rust-overlay.overlays.default (
        final: _prev: {
          rikka-fonts = import ./nix/packages/fonts.nix final;

          rikka-fonts-conf = final.makeFontsConf { fontDirectories = final.rikka-fonts; };

          rikka-gecko = final.callPackage ./nix/dist/gecko.nix { };
        }
      );

      devShells.${system}.default = pkgs.callPackage ./nix/shell.nix { };

      packages.${system} =
        let
          bazel = pkgs.callPackage ./nix/bazel.nix { env = pkgs.callPackage ./nix/env.nix { }; };
        in
        {
          rbe-image = bazel.image;
          rbe-languages-image = bazel.languagesImage;
          bazel-toolchain = bazel.toolchain;
        };

      formatter.${system} = pkgs.nixfmt-tree;
    };
}
