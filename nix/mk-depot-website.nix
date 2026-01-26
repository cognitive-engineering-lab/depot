{
  lib,
  stdenv,
  cacert,
  nodejs_22,
  pnpm_9,
  depot,
}:

{
  pname,
  version,
  src,
  pnpmHash, # The checksum for pnpm-lock.yaml
  distDir ? "dist", # Where the build output lands (relative to root)
  extraNativeBuildInputs ? [ ],
  ...
}@args:

stdenv.mkDerivation (
  finalAttrs:
  (builtins.removeAttrs args [
    "pnpmHash"
    "distDir"
    "extraNativeBuildInputs"
  ])
  // {
    nativeBuildInputs = [
      cacert
      pnpm_9
      nodejs_22
      depot
    ]
    ++ extraNativeBuildInputs;

    pnpmDeps = pnpm_9.fetchDeps {
      inherit (finalAttrs) pname version src;
      fetcherVersion = 2;
      hash = pnpmHash;
    };

    buildPhase = ''
      set -euo pipefail

      # 1. Setup PNPM Store
      export NPM_CONFIG_OFFLINE=true
      export PNPM_WRITABLE_STORE=$(mktemp -d)
      cp -LR ${finalAttrs.pnpmDeps}/* $PNPM_WRITABLE_STORE/ || true
      chmod -R +w $PNPM_WRITABLE_STORE
      export npm_config_store_dir=$PNPM_WRITABLE_STORE

      # 2. Run Depot Build
      depot b --release
    '';

    installPhase = ''
      mkdir -p $out
      cp -r ${distDir}/* $out/ 
    '';
  }
)
