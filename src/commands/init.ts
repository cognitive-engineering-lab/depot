import fs from "fs-extra";
import path from "path";

import {
  Command,
  Registration,
  binPath,
  modulesPath,
  symlinkExists,
} from "../common";
import {
  ConfigFile,
  configsFor,
  ensureConfig,
  modifyGitignore,
} from "../config-files";
import { Package, Workspace } from "../workspace";

interface InitFlags {}

export class InitCommand implements Command {
  constructor(readonly flags: InitFlags) {}

  async ensureConfigs(cfgs: ConfigFile[], dir: string) {
    let promises = cfgs.map(cfg => ensureConfig(cfg, dir));
    await Promise.all(promises);
    await modifyGitignore(cfgs, dir);
  }

  parallel() {
    return true;
  }

  async run(pkg: Package): Promise<boolean> {
    await this.ensureConfigs(configsFor(pkg), pkg.dir);
    return true;
  }

  async runWorkspace(ws: Workspace): Promise<boolean> {
    let cfgs = configsFor(ws);
    if (!ws.monorepo) cfgs = cfgs.concat(configsFor(ws.packages[0]));
    await this.ensureConfigs(cfgs, ws.root);

    let pnpmPath = path.join(binPath, "pnpm");
    let success = await ws.spawn({ script: pnpmPath, opts: ["install"] });
    if (!success) return false;

    let typesDir = ws.path(path.join("node_modules", "@types"));
    let jestTypesDir = path.join(typesDir, "jest");
    if (!(await symlinkExists(jestTypesDir))) {
      await fs.mkdirp(typesDir);
      await fs.symlink(path.join(modulesPath, "@types", "jest"), jestTypesDir);
    }

    return true;
  }

  static register: Registration = program => program.command("init");
}
