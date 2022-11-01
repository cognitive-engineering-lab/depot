import esbuild from "esbuild";
import fs from "fs-extra";

let manifest = JSON.parse(fs.readFileSync("package.json"));
let debug = process.argv.includes("-g");
let watch = process.argv.includes("-w");
esbuild.build({
  entryPoints: ["src/main.ts"],
  outdir: "dist",
  bundle: true,
  minify: !debug,
  platform: "node",
  external: Object.keys(manifest.dependencies),
  sourcemap: debug,
  watch,
  plugins: [
    {
      name: "esm-externals",
      setup(build) {
        build.onResolve({ filter: /^sort-package-json$/ }, args => ({
          path: args.path,
          namespace: "esm-externals",
        }));
        build.onResolve({ filter: /.*/, namespace: "esm-externals" }, args => ({
          path: args.path,
          external: true,
        }));
        build.onLoad({ filter: /.*/, namespace: "esm-externals" }, args => ({
          contents: `var path = "${args.path}"; var module = import(path); export default module;`,
        }));
      },
    },
  ],
});

fs.copy("src/assets", "dist/assets", { recursive: true });
