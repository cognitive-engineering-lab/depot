import esbuild from "esbuild";
import fs from "fs-extra";
import _ from "lodash";

let manifest = JSON.parse(fs.readFileSync("package.json"));
let watch = process.argv.includes("-w");
let devMode = watch;

esbuild.build({
  entryPoints: ["src/main.ts", "src/compile-entrypoint.ts"],
  outdir: "dist",
  bundle: true,
  platform: "node",
  format: "esm",
  outExtension: { ".js": ".mjs" },
  external: Object.keys(manifest.dependencies),
  sourcemap: watch,
  define: {
    DEV_MODE: JSON.stringify(devMode),
  },
  watch,
  plugins: [
    {
      name: "executable",
      setup(build) {
        build.onEnd(async () => {
          let p = "dist/main.mjs";
          // This originally used the `banner` option in esbuild, but apparently
          // the "use strict" invocation is always put before the banner when format = CJS,
          // so we have to manually write it ourselves.
          let f = await fs.promises.open(p, "r+");
          await f.chmod(fs.constants.S_IRWXU);
          let contents = await f.readFile("utf-8");
          await f.write(`#!/usr/bin/env node\n${contents}`, 0);
          await f.close();
        });
      },
    },
  ],
});

fs.copy("src/assets", "dist/assets", { recursive: true });
