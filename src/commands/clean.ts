import fs from "fs-extra";

import { Command, Registration } from "../common";
import { log } from "../log";
import { Package, Workspace } from "../workspace";

interface CleanFlags {}

export class CleanCommand implements Command {
  constructor(readonly flags: CleanFlags) {}

  parallel() {
    return true;
  }

  async rmDirs(dirs: string[]) {
    log.info(`Removing:\n${dirs.join("\n")}`);
    return await Promise.all(
      dirs.map(d => fs.rm(d, { recursive: true, force: true }))
    );
  }

  async run(pkg: Package): Promise<boolean> {
    let dirs = ["dist", "node_modules"];
    await this.rmDirs(dirs.map(d => pkg.path(d)));
    return true;
  }

  async runWorkspace(ws: Workspace): Promise<boolean> {
    let dirs = ["node_modules"];
    await this.rmDirs(dirs.map(d => ws.path(d)));
    return true;
  }

  static register: Registration = program =>
    program.command("clean").description("Delete Graco-generated files");
}
