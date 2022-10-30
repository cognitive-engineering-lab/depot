import * as cp from "child_process";
import esbuild, { Plugin } from "esbuild";
import path from "path";
import fs from "fs-extra";
import { sassPlugin } from "esbuild-sass-plugin";
import { IDependencyMap, IPackageJson } from "package-json-type";
import * as commander from "commander";

import { binPath, findJsFile, getManifest } from "./common";
import { Command } from "./command";

interface BuildFlags {
  watch?: boolean;
  release?: boolean;
}

// Detects ANSI codes in a string. Taken from https://github.com/chalk/ansi-regex
const ANSI_REGEX = new RegExp(
  [
    "[\\u001B\\u009B][[\\]()#;?]*(?:(?:(?:(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]+)*|[a-zA-Z\\d]+(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]*)*)?\\u0007)",
    "(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-nq-uy=><~]))",
  ].join("|"),
  "g"
);

export class BuildCommand extends Command {
  constructor(readonly flags: BuildFlags, readonly manifest: IPackageJson) {
    super();
  }

  async check(): Promise<boolean> {
    this.configManager.ensureConfig("tsconfig.json");

    let tscPath = path.join(binPath, "tsc");

    let opts = ["-emitDeclarationOnly"];
    if (this.flags.watch) {
      opts.push("-w");
    }

    let tsc = cp.spawn(tscPath, opts);
    tsc.stdout!.on("data", (data) => {
      // Get rid of ANSI codes so the terminal isn't randomly cleared by tsc's output.
      console.log(data.toString().replace(ANSI_REGEX, "").trim());
    });
    let exitCode: number = await new Promise((resolve) => {
      tsc.on("exit", resolve);
    });
    return exitCode == 0;
  }

  async compileLibrary(entry: string): Promise<boolean> {
    let keys = (map?: IDependencyMap) => Object.keys(map || {});
    let external = keys(this.manifest.peerDependencies).concat(
      keys(this.manifest.dependencies)
    );

    let plugins: Plugin[] = [
      sassPlugin(),
      {
        name: "files",
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
      },
    ];

    let result = await esbuild.build({
      entryPoints: [entry],
      format: "esm",
      outdir: "dist",
      bundle: true,
      watch: this.flags.watch,
      minify: this.flags.release,
      sourcemap: !this.flags.release,
      external,
      plugins,
    });

    return result.errors.length == 0;
  }

  async compileWebsite(entry: string): Promise<boolean> {
    this.configManager.ensureConfig("vite.config.ts");

    let vitePath = path.join(binPath, "vite");

    let opts = ["build", "--minify=false"];
    if (this.flags.watch) {
      opts.push("-w");
    }

    let vite = cp.spawn(vitePath, opts);
    vite.stdout!.on("data", (data) => {
      // Get rid of ANSI codes so the terminal isn't randomly cleared by tsc's output.
      console.log(data.toString() /*.replace(ANSI_REGEX, "").trim()*/);
    });
    vite.stderr!.on("data", (data) => {
      console.error(data.toString());
    });
    let exitCode: number = await new Promise((resolve) => {
      vite.on("exit", resolve);
    });
    return exitCode == 0;
  }

  compile(): Promise<boolean> {
    let lib = findJsFile("src/lib");
    if (lib) return this.compileLibrary(lib);

    let index = findJsFile("src/index");

    if (index) return this.compileWebsite(index);

    throw new Error("No valid entry point");
  }

  async run(): Promise<boolean> {
    await fs.rm("dist", { recursive: true, force: true });

    let results = await Promise.all([this.check(), this.compile()]);

    let buildPath = "build.mjs";
    if (fs.existsSync(buildPath))
      await import(path.join(process.cwd(), buildPath));

    return results.every((x) => x);
  }

  static register(program: commander.Command) {
    program
      .command("build")
      .option("-w, --watch", "Watch for changes and rebuild")
      .option("-r, --release", "Build for production")
      .action((flags) => new BuildCommand(flags, getManifest()).main());
  }
}
