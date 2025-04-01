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
        depot-js = pkgs.rustPlatform.buildRustPackage {
          pname = manifest.name;
          version = manifest.version;
          cargoLock.lockFile = ./Cargo.lock;
          src = pkgs.lib.cleanSource ./.;    
          nativeBuildInputs = [ pkgs.git ];
          # NOTE: depot's tests have a lot of external dependencies: node, biome, ...
          # We'll ignore them for now and figure it out later ;)
          doCheck = false;
        };

        ci-check = pkgs.writeScriptBin "ci-check" ''
          cargo clippy -- -D warnings && 
          cargo fmt --check &&
          cargo test --features dev -- --test-threads=1
        '';
      in {
        packages = { default = depot-js; };

        devShells = with pkgs; rec {
          ci = mkShell {
            buildInputs = [ 
              ci-check 
              cargo 
              rustc
              clippy 
              nodejs_22 
              pnpm_9 
            ];

            RUST_BACKTRACE="1";
            RUST_LIB_BACKTRACE="1";
            TOKIO_WORKER_THREADS="1";
          };

          default = mkShell {
            inherit ci;
            buildInputs = ci.buildInputs ++ [ rust-analyzer ];
          };
        };
      });
}
