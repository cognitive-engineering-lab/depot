import * as cp from "child_process";
import * as commander from "commander";
import esbuild, { Plugin } from "esbuild";
import { sassPlugin } from "esbuild-sass-plugin";
import fs from "fs-extra";
import { IDependencyMap, IPackageJson } from "package-json-type";
import path from "path";
import blessed from "blessed";
import * as pty from "node-pty";

import { Command } from "./command";
import { binPath, findJsFile, getManifest, modulesPath } from "./common";
import _ from "lodash";

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

class ProcessManager {
  screen = blessed.screen({
    fastCSR: true,
    terminal: "xterm-256color",
    fullUnicode: true,
  });
  boxes: blessed.Widgets.Log[];
  curBox: number = 0;

  constructor() {
    this.screen.title = "my window title";
    this.boxes = _.range(0, 2)
      .map((x) =>
        _.range(0, 2).map((y) =>
          blessed.log({
            top: x == 0 ? "0" : "50%",
            left: y == 0 ? "0" : "50%",
            width: "50%",
            height: "50%",
            content: "",
            border: {
              type: "line",
            },
            style: {
              border: {
                fg: "#f0f0f0",
              },
            },
          })
        )
      )
      .flat();

    this.boxes.forEach((box) => this.screen.append(box));

    // Quit on Escape, q, or Control-C.
    this.screen.key(["escape", "q", "C-c"], function (ch, key) {
      return process.exit(0);
    });
  }

  start() {
    this.screen.render();
  }

  async spawn(script: string, opts: string[]): Promise<boolean> {
    let box = this.boxes[this.curBox];
    box.setLabel(path.basename(script));
    this.curBox += 1;

    let p = pty.spawn(script, opts, {
      env: {
        ...process.env,
        NODE_PATH: modulesPath,
      },
    });
    p.onData((data) => box.log(data));
    let exitCode: number = await new Promise((resolve) => {
      p.onExit(({ exitCode }) => resolve(exitCode));
    });
    return exitCode == 0;
  }
}

export class BuildCommand extends Command {
  processManager = new ProcessManager();

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

    return this.processManager.spawn(tscPath, opts);
  }

  async lint(): Promise<boolean> {
    this.configManager.ensureConfig(".eslintrc.js");

    let eslintPath = path.join(binPath, "eslint");
    let opts = ["--ext", "js,ts,tsx", "src"];

    return this.processManager.spawn(eslintPath, opts);
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

    this.processManager.start();
    let results = await Promise.all([
      this.check(),
      this.lint(),
      this.compile(),
    ]);

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
