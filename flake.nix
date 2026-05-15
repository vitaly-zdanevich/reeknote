{
  description = "Command-line Evernote client";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          reeknote = pkgs.callPackage ./nix/package.nix { };
        in
        {
          inherit reeknote;
          default = reeknote;
        }
      );

      apps = forAllSystems (
        system:
        let
          package = self.packages.${system}.reeknote;
        in
        {
          reeknote = {
            type = "app";
            program = "${package}/bin/reeknote";
          };
          rnsync = {
            type = "app";
            program = "${package}/bin/rnsync";
          };
          default = self.apps.${system}.reeknote;
        }
      );

      checks = forAllSystems (system: {
        reeknote = self.packages.${system}.reeknote;
      });
    };
}
