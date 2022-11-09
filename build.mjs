import esbuild from "esbuild";
import fs from "fs-extra";
import _ from "lodash";
import path from "path";
import { fileURLToPath } from "url";

let repoRoot = path.dirname(fileURLToPath(import.meta.url));
let manifest = JSON.parse(fs.readFileSync("package.json"));
let watch = process.argv.includes("-w");
let debug = process.argv.includes("-g") || watch;

esbuild.build({
  entryPoints: ["src/main.ts"],
  outdir: "dist",
  bundle: true,
  minify: !debug,
  platform: "node",
  format: "esm",
  outExtension: { ".js": ".mjs" },
  external: Object.keys(manifest.dependencies),
  sourcemap: debug,
  define: { REPO_ROOT: JSON.stringify(repoRoot) },
  watch,
});

fs.copy("src/assets", "dist/assets", { recursive: true });
