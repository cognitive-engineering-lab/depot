import type esbuild from "esbuild";
import fs from "fs-extra";
import indentString from "indent-string";
import _ from "lodash";
import type { IPackageJson } from "package-json-type";
import path from "path";

import { Command, SpawnProps, spawn } from "./common";
import { log } from "./log";

export const PLATFORMS = ["browser", "node"] as const;
export type Platform = typeof PLATFORMS[number];
export const TARGETS = ["bin", "lib"] as const;
export type Target = typeof TARGETS[number];

export interface GracoConfig {
  format?: esbuild.Format;
}

export class Package {
  readonly platform: Platform;
  readonly target: Target;
  readonly name: string;
  readonly entryPoint: string;

  constructor(
    readonly dir: string,
    readonly manifest: IPackageJson & { graco?: GracoConfig }
  ) {
    this.name = manifest.name || path.basename(dir);

    let entryPoint;
    if ((entryPoint = this.findJs("lib"))) {
      this.platform = "node";
      this.target = "lib";
      this.entryPoint = entryPoint;
    } else if ((entryPoint = this.findJs("main"))) {
      this.platform = "node";
      this.target = "bin";
      this.entryPoint = entryPoint;
    } else if ((entryPoint = this.findJs("index"))) {
      this.platform = "browser";
      this.target = "bin";
      this.entryPoint = entryPoint;
    } else {
      throw new Error(`Could not determine platform for package: ${this.name}`);
    }
  }

  config(): GracoConfig {
    return this.manifest.graco || {};
  }

  static async load(dir: string): Promise<Package> {
    dir = path.resolve(dir);
    let manifest;
    try {
      manifest = JSON.parse(
        await fs.readFile(path.join(dir, "package.json"), "utf-8")
      );
    } catch (e: any) {
      let err = indentString(e.toString(), 4);
      throw new Error(
        `Failed to read package.json for package \`${dir}\`\n${err}`
      );
    }

    return new Package(dir, manifest);
  }

  findJs = (basename: string): string | undefined => {
    let exts = ["tsx", "ts", "js"];
    return exts
      .map(e => path.join(this.dir, "src", `${basename}.${e}`))
      .find(fs.existsSync);
  };

  path(base: string): string {
    return path.join(this.dir, base);
  }

  spawn(props: Omit<SpawnProps, "cwd">): Promise<boolean> {
    return spawn({ ...props, cwd: this.dir });
  }
}

type DepGraph = { [name: string]: string[] };

let getGitRoot = async (): Promise<string | undefined> => {
  let gitRoot: string[] = [];
  let success = await spawn({
    script: "git",
    opts: ["rev-parse", "--show-toplevel"],
    cwd: process.cwd(),
    onData: data => gitRoot.push(data),
  });
  return success ? gitRoot.join("").trim() : undefined;
};

let findWorkspaceRoot = (gitRoot: string): string | undefined => {
  let pathToCwd = path.relative(gitRoot, path.resolve("."));
  let components = pathToCwd.split(path.sep);
  let i = _.range(components.length).find(i =>
    fs.existsSync(path.join(gitRoot, ...components.slice(i), "package.json"))
  );
  if (i !== undefined) return path.join(gitRoot, ...components.slice(i));
};

export class Workspace {
  pkgMap: { [name: string]: Package };
  depGraph: DepGraph;

  constructor(
    public readonly root: string,
    public readonly packages: Package[],
    public readonly monorepo: boolean
  ) {
    this.pkgMap = _.fromPairs(packages.map(pkg => [pkg.name, pkg]));
    this.depGraph = this.buildDepGraph();
  }

  static async load() {
    let gitRoot = await getGitRoot();
    let root: string | undefined;
    if (gitRoot) {
      root = findWorkspaceRoot(gitRoot);
    } else {
      let cwd = path.resolve(".");
      if (fs.existsSync(path.join(cwd, "package.json"))) root = cwd;
    }
    if (!root) throw new Error(`Could not find workspace`);
    log.debug(`Workspace root: ${root}`);

    let pkgDir = path.join(root, "packages");
    let monorepo = fs.existsSync(pkgDir);
    log.debug(`Workspace is monorepo: ${monorepo}`);

    let packages = await Promise.all(
      monorepo
        ? fs.readdirSync(pkgDir).map(d => Package.load(path.join(pkgDir, d)))
        : [Package.load(root)]
    );
    log.debug(`Found packages: [${packages.map(p => p.name).join(", ")}]`);

    return new Workspace(root, packages, monorepo);
  }

  buildDepGraph(): DepGraph {
    let rootSet = new Set(Object.keys(this.pkgMap));
    let depGraph = _.fromPairs(
      [...rootSet].map(name => {
        let manifest = this.pkgMap[name].manifest;
        let allVersionedDeps = [
          manifest.dependencies,
          manifest.devDependencies,
          manifest.peerDependencies,
        ];
        return [
          name,
          new Set(
            allVersionedDeps
              .map(deps => Object.keys(deps || {}))
              .flat()
              .filter(name => rootSet.has(name))
          ),
        ];
      })
    );

    let union = <T>(a: Set<T>, b: Set<T>): boolean => {
      let n = a.size;
      b.forEach(x => a.add(x));
      return a.size > n;
    };
    while (true) {
      let changed = false;
      Object.keys(depGraph).forEach(name => {
        let deps = [...depGraph[name]];
        deps.forEach(dep => {
          changed = union(depGraph[name], depGraph[dep]) || changed;
        });
      });
      if (!changed) break;
    }
    return _.fromPairs(
      Object.keys(depGraph).map(name => [name, [...depGraph[name]]])
    );
  }

  dependencyClosure(roots: string[]): Package[] {
    let depsSet = new Set([...roots]);
    while (true) {
      let n = depsSet.size;
      [...depsSet].forEach(p => {
        this.depGraph[p].forEach(p2 => depsSet.add(p2));
      });
      if (depsSet.size == n) break;
    }
    return [...depsSet].map(p => this.pkgMap[p]);
  }

  async runPackages(cmd: Command, only?: string[]): Promise<boolean> {
    let rootSet = only || this.packages.map(p => p.name);
    let pkgs = this.dependencyClosure(rootSet);

    if (cmd.parallel && cmd.parallel()) {
      let results = await Promise.all(pkgs.map(pkg => cmd.run!(pkg)));
      return results.every(x => x);
    }

    let status: { [name: string]: "queued" | "running" | "finished" } =
      _.fromPairs(pkgs.map(pkg => [pkg.name, "queued"]));
    let canExecute = () =>
      pkgs.filter(
        pkg =>
          status[pkg.name] == "queued" &&
          this.depGraph[pkg.name].every(name => status[name] == "finished")
      );
    let promise = new Promise<void>((resolve, reject) => {
      let runTasks = () =>
        canExecute().forEach(async pkg => {
          console.log("Running task for:", pkg.name);
          status[pkg.name] = "running";
          let success = await cmd.run!(pkg);
          if (!success) reject();
          status[pkg.name] = "finished";

          if (Object.keys(status).every(k => status[k] == "finished"))
            resolve();
          else runTasks();
        });
      runTasks();
    });
    try {
      await promise;
      return true;
    } catch (e) {
      return false;
    }
  }

  async run(cmd: Command, only?: string[]): Promise<boolean> {
    let success = true;
    if (cmd.run) success = (await this.runPackages(cmd, only)) && success;
    if (cmd.runWorkspace) success = (await cmd.runWorkspace(this)) && success;
    return success;
  }

  spawn(props: Omit<SpawnProps, "cwd">): Promise<boolean> {
    return spawn({ ...props, cwd: this.root });
  }

  path(base: string): string {
    return path.join(this.root, base);
  }
}
