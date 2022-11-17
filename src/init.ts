import fs from "fs-extra";
import path from "path";

import {
  Command,
  Registration,
  binPath,
  modulesPath,
  symlinkExists,
} from "./common";
import {
  CONFIG_FILE_DIR,
  ConfigFile,
  configsFor,
  modifyGitignore,
} from "./config-files";
import { Package, Workspace } from "./workspace";

interface InitFlags {}

export class InitCommand implements Command {
  constructor(readonly flags: InitFlags) {}

  async ensureConfigs(cfgs: ConfigFile[], dir: string) {
    let promises = cfgs.map(async config => {
      let srcPath = path.join(CONFIG_FILE_DIR, config.name);
      let dstPath = path.join(dir, config.name);
      if (await symlinkExists(dstPath)) return;
      await fs.symlink(srcPath, dstPath);
      console.log(`Linked: ${dstPath}`);
    });
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
    await this.ensureConfigs(configsFor(ws), ws.root);

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
