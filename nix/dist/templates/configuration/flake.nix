{
  description = "NixOS configuration with the Rikka desktop";

  inputs = {
    rikka.url = "github:project-rikka/packages";
    nixpkgs.follows = "rikka/nixpkgs";
  };

  outputs =
    { nixpkgs, rikka, ... }@inputs:
    let
      system = nixpkgs.lib.nixosSystem {
        specialArgs = { inherit inputs; };
        modules = [
          rikka.nixosModules.default
          ./configuration.nix
          ./rikka
        ];
      };
    in
    {
      nixosConfigurations.${system.config.networking.hostName} = system;
    };
}
