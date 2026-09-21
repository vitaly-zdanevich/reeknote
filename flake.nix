{
  description = "Command-line Evernote client";

  inputs = {
		# Includes the direct-CDN crate downloader, avoiding crates.io API 403s.
		nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
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
