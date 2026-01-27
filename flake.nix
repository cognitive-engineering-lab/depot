{
  description = "A JS devtool orchestrator";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    let
      depotOverlay = final: prev: {
        depot = final.callPackage ./nix/package.nix {
          cleanSource = prev.lib.cleanSource;
        };

        mkDepotPackage = final.callPackage ./nix/mk-depot-package.nix {
          depot = final.depot;
        };
      };
    in
    {
      overlays.default = depotOverlay;
    }
    // flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ depotOverlay ];
        };
      in
      {
        packages = {
          default = pkgs.depot;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ pkgs.depot ];
        };
      }
    );
}
