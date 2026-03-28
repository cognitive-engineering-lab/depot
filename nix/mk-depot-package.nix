{
  lib,
  stdenv,
  cacert,
  nodejs_22,
  pnpm_9,
  fetchPnpmDeps,
  pnpmConfigHook,
  depot,
}:

{
  pname,
  version,
  src,
  pnpmHash,
  distDir ? "dist",
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
    pnpmDeps = fetchPnpmDeps {
      inherit (finalAttrs) pname version src;
      hash = pnpmHash;
      fetcherVersion = 3;
    };

    nativeBuildInputs = [
      cacert
      pnpm_9
      nodejs_22
      pnpmConfigHook
      depot
    ]
    ++ extraNativeBuildInputs;

    buildPhase = ''
      runHook preBuild

      # Run Depot Build
      depot --no-fullscreen b --release

      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall

      mkdir -p $out
      cp -r ${distDir}/* $out/ 

      runHook postInstall
    '';
  }
)
