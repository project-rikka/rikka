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

      templates = {
        default = {
          path = ./templates/configuration;
          description = "A NixOS flake with the Rikka desktop and the modules Rikka writes";
          welcomeText = ''
            # Rikka

            `flake.nix` builds `./configuration.nix` with Rikka, and `rikka/` holds the
            modules Rikka's Settings and Packages write. Set `rikka.enable = true;` in
            `configuration.nix`, then run `sudo nixos-rebuild switch --flake /etc/nixos`.
          '';
        };

        modules = {
          path = ./templates/configuration/rikka;
          description = "The modules Rikka's Settings and Packages write, for an existing flake";
          welcomeText = ''
            # Rikka's modules

            Import this directory from your NixOS configuration. When it is not
            `/etc/nixos/rikka`, set `rikka.configuration.flake` and
            `rikka.configuration.modules` to where your flake and this directory are.
          '';
        };
      };

      checks.${system}.module =
        (nixpkgs.lib.nixosSystem {
          modules = [
            self.nixosModules.default
            ./templates/configuration/rikka
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
