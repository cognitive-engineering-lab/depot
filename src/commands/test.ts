import fs from "fs-extra";
import path from "path";

import { Command, Registration, binPath } from "../common";
import { Package } from "../workspace";

interface TestFlags {}

export class TestCommand implements Command {
  constructor(readonly flags: TestFlags) {}

  async run(pkg: Package): Promise<boolean> {
    if (!fs.existsSync(pkg.path("jest.config.cjs"))) return true;

    let jestPath = path.join(binPath, "jest");
    return pkg.spawn({ script: jestPath, opts: [] });
  }

  static register: Registration = program => program.command("test");
}
