import fs from "fs-extra";
import path from "path";

import { Command, Registration, binPath } from "../common";
import { Workspace } from "../workspace";

interface TestFlags {}

export class TestCommand implements Command {
  constructor(readonly flags: TestFlags) {}

  async runWorkspace(ws: Workspace): Promise<boolean> {
    if (!fs.existsSync(ws.path("jest.config.cjs"))) return true;

    let jestPath = path.join(binPath, "jest");
    return ws.spawn({ script: jestPath, opts: ["--passWithNoTests"] });
  }

  static register: Registration = program =>
    program.command("test").description("Run tests");
}
