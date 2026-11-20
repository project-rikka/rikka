{
  description = "Rikka: prebuilt packages and the NixOS module, published by project-rikka/rikka";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      release = builtins.fromJSON (builtins.readFile ./release.json);

      inherit (release) system;

      packages = import ./packages.nix {
        pkgs = nixpkgs.legacyPackages.${system};
        inherit release;
      };
    in
    {
      packages.${system} = packages;

      catalog = builtins.mapAttrs (_: entry: {
        inherit (entry)
          name
          summary
          offer
          version
          ;
      }) release.components;

      nixosModules.default = import ./module.nix { inherit release packages; };

      checks.${system}.module =
        (nixpkgs.lib.nixosSystem {
          modules = [
            self.nixosModules.default
            {
              nixpkgs.hostPlatform = system;
              boot.loader.systemd-boot.enable = true;
              fileSystems."/" = {
                device = "/dev/disk/by-label/nixos";
                fsType = "ext4";
              };
              users.users.rikka.isNormalUser = true;
              rikka = {
                enable = true;
                autologin.user = "rikka";
                apps = builtins.mapAttrs (_: _: { enable = true; }) (
                  nixpkgs.lib.filterAttrs (_: entry: entry.offer != "required") release.components
                );
              };
              system.stateVersion = nixpkgs.lib.trivial.release;
            }
          ];
        }).config.system.build.toplevel;

      formatter.${system} = nixpkgs.legacyPackages.${system}.nixfmt-tree;
    };
}
