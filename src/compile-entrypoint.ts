import esbuild, { Plugin } from "esbuild";
import { sassPlugin } from "esbuild-sass-plugin";
import { IDependencyMap } from "package-json-type";
import fs from "fs-extra";
import path from "path";

async function main() {
  let manifest = JSON.parse(await fs.readFile("package.json", "utf-8"));
  let keys = (map?: IDependencyMap) => Object.keys(map || {});
  let external = keys(manifest.peerDependencies).concat(
    keys(manifest.dependencies)
  );

  let pathExtensionLoaderPlugin: Plugin = {
    name: "path-extension-loader",
    setup(build) {
      let loaders = ["url", "raw"];
      loaders.forEach((loader) => {
        let filter = new RegExp(`\\?${loader}$`);
        build.onResolve({ filter }, (args) => {
          let p = args.path.slice(0, -(loader.length + 1));
          p = path.resolve(path.join(args.resolveDir, p));
          return { path: p, namespace: loader };
        });
      });

      let toCopy = new Set<string>();
      build.onLoad({ filter: /.*/, namespace: "url" }, (args) => {
        toCopy.add(args.path);
        let url = JSON.stringify("./" + path.basename(args.path));
        let contents = `export default new URL(${url}, import.meta.url);`;
        return { contents, loader: "js" };
      });
      build.onEnd(() => {
        toCopy.forEach((p) => {
          fs.copyFileSync(
            p,
            path.join(build.initialOptions.outdir!, path.basename(p))
          );
        });
      });

      build.onLoad({ filter: /.*/, namespace: "raw" }, (args) => {
        let contents = fs.readFileSync(args.path, "utf-8");
        return { contents, loader: "text" };
      });
    },
  };

  let plugins: Plugin[] = [sassPlugin(), pathExtensionLoaderPlugin];

  let loader: { [ext: string]: esbuild.Loader } = {
    ".otf": "file",
    ".woff": "file",
    ".woff2": "file",
    ".ttf": "file",
    ".wasm": "file",
    ".bib": "text",
    ".png": "file",
    ".jpg": "file",
    ".jpeg": "file",
    ".gif": "file",
  };

  let esbuildConfigPath = path.join(process.cwd(), "esbuild.config.mjs");
  let externalConfig: esbuild.BuildOptions = {};
  if (fs.existsSync(esbuildConfigPath)) {
    let configModule = await import(esbuildConfigPath);
    externalConfig = configModule.default;
  }

  let entryPoint = process.argv[2];
  let watch = process.argv.includes("-w");
  let release = process.argv.includes("--release");

  try {
    await esbuild.build({
      ...externalConfig,
      entryPoints: [entryPoint],
      format: (manifest.graco && manifest.graco.format) || "esm",
      outdir: "dist",
      bundle: true,
      watch,
      minify: release,
      sourcemap: !release,
      external,
      plugins,
      loader,
    });
  } catch (e) {
    process.exit(1);
  }
}

main();
