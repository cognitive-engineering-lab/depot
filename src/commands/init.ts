import fs from "fs-extra";
import path from "path";

import {
  Command,
  Registration,
  binPath,
  modulesPath,
  symlinkExists,
} from "../common";
import { Workspace } from "../workspace";

interface InitFlags {}

export class InitCommand implements Command {
  constructor(readonly flags: InitFlags) {}

  async runWorkspace(ws: Workspace): Promise<boolean> {
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

  static register: Registration = program =>
    program.command("init").description("Setup config files in workspace");
}
