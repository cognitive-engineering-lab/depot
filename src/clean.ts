import fs from "fs-extra";

import { Command, Registration } from "./common";
import { configsFor, findManagedConfigs } from "./config-files";
import { Package, Workspace } from "./workspace";

interface CleanFlags {
  all?: boolean;
}

export class CleanCommand implements Command {
  constructor(readonly flags: CleanFlags) {}

  parallel() {
    return true;
  }

  async rmDirs(dirs: string[]) {
    return await Promise.all(
      dirs.map(d => fs.rm(d, { recursive: true, force: true }))
    );
  }

  async run(pkg: Package): Promise<boolean> {
    let dirs = ["dist", "node_modules"];
    if (this.flags.all) {
      let cfgs = await findManagedConfigs(configsFor(pkg), pkg.dir);
      dirs = dirs.concat(cfgs.map(cfg => cfg.name))
    }
    await this.rmDirs(dirs.map(d => pkg.path(d)));
    return true;
  }

  async runWorkspace(ws: Workspace): Promise<boolean> {
    let dirs = ["node_modules"];
    if (this.flags.all) {
      let cfgs = await findManagedConfigs(configsFor(ws), ws.root);
      dirs = dirs.concat(cfgs.map(cfg =>cfg.name));
    }
    await this.rmDirs(dirs.map(d => ws.path(d)));
    return true;
  }

  static register: Registration = program =>
    program
      .command("clean")
      .option("-a, --all", "Clean up all Graco files (including config files)");
}
