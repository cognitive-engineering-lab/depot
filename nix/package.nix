{
  lib,
  rustPlatform,
  git,
  cleanSource,
}:

let
  manifest = (lib.importTOML ../crates/depot/Cargo.toml).package;
in
rustPlatform.buildRustPackage {
  pname = manifest.name;
  version = manifest.version;
  cargoLock.lockFile = ../Cargo.lock;
  src = cleanSource ../.;
  nativeBuildInputs = [ git ];
  doCheck = false;
}
