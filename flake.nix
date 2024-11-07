{
  description = "A JS devtool orchestrator";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
  flake-utils.lib.eachDefaultSystem (system:
  let
    manifest = (pkgs.lib.importTOML ./crates/depot/Cargo.toml).package;
    pkgs = import nixpkgs { inherit system; };
    depot-js = pkgs.rustPlatform.buildRustPackage rec {
      pname = manifest.name;
      version = manifest.version;
      cargoLock.lockFile = ./Cargo.lock;
      src = pkgs.lib.cleanSource ./.;    
      nativeBuildInputs = [ pkgs.git ];
      # NOTE: depot's tests have a lot of external dependencies: node, biome, ...
      # We'll ignore them for now and figure it out later ;)
      doCheck = false;
    };
  in {
    packages = {
      default = depot-js;
    };
  });
}
