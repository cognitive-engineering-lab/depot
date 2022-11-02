import blessed from "blessed";
import chalk from "chalk";
import esbuild, { Plugin } from "esbuild";
import { sassPlugin } from "esbuild-sass-plugin";
import fs from "fs-extra";
import { createServer } from "http-server";
import _ from "lodash";
import { IDependencyMap } from "package-json-type";
import path from "path";

import { Command, Registration, binPath } from "./common";
import { Package } from "./workspace";

interface BuildFlags {
  watch?: boolean;
  release?: boolean;
}

abstract class Logger {
  start() {}
  abstract log(name: string, data: string): void;
  end() {}
}

class OnceLogger extends Logger {
  logs: {
    name: string;
    logs: string[];
  }[];

  constructor(boxes: [string, number][]) {
    super();
    this.logs = boxes.map(([name]) => ({ name, logs: [] }));
  }

  log(name: string, data: string) {
    let entry = this.logs.find(r => r.name == name);
    if (!entry) throw new Error(`No logger named: ${name}`);
    entry.logs.push(data);
  }

  end() {
    this.logs.forEach(({ name, logs }) => {
      console.log(chalk.bold(name) + "\n");
      logs.forEach(log => console.log(log));
      console.log(chalk.bold(".".repeat(80)) + "\n");
    });
  }
}

class WatchLogger extends Logger {
  screen = blessed.screen({
    fastCSR: true,
    terminal: "xterm-256color",
    fullUnicode: true,
  });
  boxes: { [name: string]: blessed.Widgets.Log } = {};

  constructor(boxes: [string, number][]) {
    super();
    this.screen.title = "my window title";

    this.boxes = {};
    let left = 0;
    let top = 0;
    let width = this.screen.width as number;
    let hh = Math.floor(((this.screen.height as number) * 2) / 3);
    boxes.forEach(([name, frac]) => {
      let boxWidth = Math.floor(frac * width);
      let box = blessed.log({
        top,
        left,
        width: boxWidth,
        height: hh,
        border: { type: "line" },
        style: { border: { fg: "#eeeeee" } },
        label: name,
      });

      left += boxWidth;
      if (left >= width) {
        left = 0;
        top = hh;
      }

      this.boxes[name] = box;
      this.screen.append(box);
    });

    // Quit on Escape, q, or Control-C.
    this.screen.key(["escape", "q", "C-c"], function (ch, key) {
      return process.exit(0);
    });
  }

  log(name: string, data: string) {
    let box = this.boxes[name];

    // Hacky support for ANSI codes that Vite uses.
    // Note: blessed-xterm solves this issue, but is build in a way
    // that conflicts with other plugins (it pollutes global namespace
    // which breaks sass). Need to find a more durable solution.
    if (data.includes("[2K")) box.deleteBottom();
    else box.log(data.replace("[1G", ""));
  }

  start() {
    this.screen.render();
  }
}

export class BuildCommand implements Command {
  logger: Logger;

  constructor(readonly flags: BuildFlags) {
    let boxes: [string, number][] = [
      ["build", 0.5],
      ["check", 0.5],
      ["lint", 0.5],
      ["script", 0.5],
      // ["graco", 0.33],
    ];
    this.logger = flags.watch ? new WatchLogger(boxes) : new OnceLogger(boxes);
  }

  async check(pkg: Package): Promise<boolean> {
    let tscPath = path.join(binPath, "tsc");

    let opts = ["-emitDeclarationOnly"];
    if (this.flags.watch) {
      opts.push("-w");
    }

    return pkg.spawn({
      script: tscPath,
      opts,
      onData: data => this.logger.log("check", data),
    });
  }

  async lint(pkg: Package): Promise<boolean> {
    let eslintPath = path.join(binPath, "eslint");
    let eslintOpts = ["--ext", "js,ts,tsx", "src"];

    let script, opts;
    if (this.flags.watch) {
      let watchPath = path.join(binPath, "watch");
      script = watchPath;
      opts = [`${eslintPath} ${eslintOpts.join(" ")}`, `src`];
    } else {
      script = eslintPath;
      opts = eslintOpts;
    }

    // TODO: flag like -Werror to make lints a failure
    await pkg.spawn({
      script,
      opts,
      onData: data => this.logger.log("lint", data),
    });
    return true;
  }

  async compileLibrary(pkg: Package): Promise<boolean> {
    let keys = (map?: IDependencyMap) => Object.keys(map || {});
    let external = keys(pkg.manifest.peerDependencies).concat(
      keys(pkg.manifest.dependencies)
    );

    let logger = this.logger;
    let plugins: Plugin[] = [
      sassPlugin(),
      {
        name: "files",
        setup(build) {
          let loaders = ["url", "raw"];
          loaders.forEach(loader => {
            let filter = new RegExp(`\\?${loader}$`);
            build.onResolve({ filter }, args => {
              let p = args.path.slice(0, -(loader.length + 1));
              p = path.resolve(path.join(args.resolveDir, p));
              return { path: p, namespace: loader };
            });
          });

          let toCopy = new Set<string>();
          build.onLoad({ filter: /.*/, namespace: "url" }, args => {
            toCopy.add(args.path);
            let url = JSON.stringify("./" + path.basename(args.path));
            let contents = `export default new URL(${url}, import.meta.url);`;
            return { contents, loader: "js" };
          });
          build.onEnd(() => {
            toCopy.forEach(p => {
              fs.copyFileSync(
                p,
                path.join(
                  pkg.dir,
                  build.initialOptions.outdir!,
                  path.basename(p)
                )
              );
            });
          });

          build.onLoad({ filter: /.*/, namespace: "raw" }, args => {
            let contents = fs.readFileSync(args.path, "utf-8");
            return { contents, loader: "text" };
          });
        },
      },
      {
        name: "logging",
        setup(build) {
          build.onEnd(result => {
            if (!result.errors.length) logger.log("build", "Build complete!");
            result.errors.forEach(error => {
              logger.log(
                "build",
                chalk.red("✘ ") +
                  chalk.whiteBright.bgRed(" ERROR ") +
                  " " +
                  chalk.bold(error.text)
              );
              if (error.location) {
                logger.log(
                  "build",
                  `\t${error.location.file}:${error.location.line}:${error.location.column}`
                );
              }
            });
            logger.log("build", "\n");
          });
        },
      },
    ];

    try {
      let result = await esbuild.build({
        entryPoints: [pkg.entryPoint],
        format: "esm",
        outdir: "dist",
        bundle: true,
        watch: this.flags.watch,
        minify: this.flags.release,
        sourcemap: !this.flags.release,
        external,
        plugins,
        logLevel: "silent",
        absWorkingDir: pkg.dir,
      });
      return result.errors.length == 0;
    } catch (e) {
      return false;
    }
  }

  async compileWebsite(pkg: Package): Promise<boolean> {
    let vitePath = path.join(binPath, "vite");

    let opts = ["build", "--minify=false"];
    if (this.flags.watch) {
      opts.push("-w");
    }

    return pkg.spawn({
      script: vitePath,
      opts,
      onData: data => this.logger.log("build", data),
    });
  }

  async buildScript(pkg: Package): Promise<boolean> {
    let buildPath = pkg.path("build.mjs");
    if (fs.existsSync(buildPath)) {
      let opts = [buildPath];
      if (this.flags.watch) opts.push("-w");
      return await pkg.spawn({
        script: "node",
        opts,
        onData: data => this.logger.log("script", data),
      });
    } else {
      return true;
    }
  }

  compile(pkg: Package): Promise<boolean> {
    if (pkg.platform == "node") return this.compileLibrary(pkg);
    /* pkg.platform == "browser" */ else return this.compileWebsite(pkg);
  }

  async serve(pkg: Package): Promise<boolean> {
    if (pkg.platform == "browser" && pkg.target == "bin" && this.flags.watch) {
      let server = createServer({
        root: pkg.path("dist"),
      });
      server.listen(8000);
    }

    return true;
  }

  parallel(): boolean {
    return this.flags.watch || false;
  }

  async run(pkg: Package): Promise<boolean> {
    await fs.mkdirp(pkg.path("dist"));

    this.logger.start();

    // http-server causes an unavoidable node warning,
    // see: https://github.com/http-party/http-server/issues/537
    // therefore we silence the warning,
    // see: https://stackoverflow.com/a/73525885
    let emit = process.emit;
    process.emit = function (name: any, data: any) {
      if (name === `warning` && typeof data === `object`) {
        return false;
      }
      return emit.apply(process, arguments as any);
    } as any;

    let results = await Promise.all([
      this.check(pkg),
      this.compile(pkg),
      this.lint(pkg),
      this.buildScript(pkg),
      this.serve(pkg),
    ]);

    this.logger.end();
    return results.every(x => x);
  }

  static register: Registration = program =>
    program
      .command("build")
      .option("-w, --watch", "Watch for changes and rebuild")
      .option("-r, --release", "Build for production");
}
