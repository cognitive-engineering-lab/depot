import fs from "fs-extra";
import _ from "lodash";
import { IPackageJson } from "package-json-type";
import path from "path";

import { Command, SpawnProps, spawn } from "./common";

export const PLATFORMS = ["browser", "node"] as const;
export type Platform = typeof PLATFORMS[number];
export const TARGETS = ["bin", "lib"] as const;
export type Target = typeof TARGETS[number];

export class Package {
  readonly platform: Platform;
  readonly target: Target;
  readonly name: string;
  readonly entryPoint: string;

  constructor(readonly dir: string, readonly manifest: IPackageJson) {
    if (!manifest.name) throw new Error(`All packages must be named`);
    this.name = manifest.name;

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

  static async load(dir: string): Promise<Package> {
    dir = path.resolve(dir);
    let manifest = JSON.parse(
      await fs.readFile(path.join(dir, "package.json"), "utf-8")
    );

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
    let root = path.resolve("."); // TODO: search for root?
    let pkgDir = path.join(root, "packages");
    let monorepo = fs.existsSync(pkgDir);
    let packages = await Promise.all(
      monorepo
        ? fs.readdirSync(pkgDir).map(d => Package.load(path.join(pkgDir, d)))
        : [Package.load(root)]
    );
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
      b.forEach(a.add);
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

  async runAllPackages(cmd: Command): Promise<boolean> {
    if (cmd.parallel && cmd.parallel()) {
      let results = await Promise.all(this.packages.map(pkg => cmd.run!(pkg)));
      return results.every(x => x);
    }

    let status: { [name: string]: "queued" | "running" | "finished" } =
      _.fromPairs(this.packages.map(pkg => [pkg.name, "queued"]));
    let canExecute = () =>
      this.packages.filter(
        pkg =>
          status[pkg.name] == "queued" &&
          this.depGraph[pkg.name].every(name => status[name] == "finished")
      );
    let promise = new Promise<void>((resolve, reject) => {
      let runTasks = () =>
        canExecute().forEach(async pkg => {
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

  async run(cmd: Command): Promise<boolean> {
    let success = true;
    if (cmd.run) success = (await this.runAllPackages(cmd)) && success;
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
