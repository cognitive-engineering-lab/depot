import esbuild from "esbuild";
import fs from "fs-extra";

let manifest = JSON.parse(fs.readFileSync("package.json"));

esbuild.build({
  entryPoints: ["src/main.ts"],
  outdir: "dist",
  bundle: true,
  minify: false,
  platform: "node",
  external: Object.keys(manifest.dependencies),
  sourcemap: true,
  watch: process.argv.includes("-w"),
});

fs.copy("src/assets", "dist/assets", { recursive: true });
