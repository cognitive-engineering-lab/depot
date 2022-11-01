import path from "path";

import { Command, Registration, binPath } from "./common";
import { Workspace } from "./workspace";

interface TestFlags {}

export class TestCommand implements Command {
  constructor(readonly flags: TestFlags) {}

  async runWorkspace(ws: Workspace): Promise<boolean> {
    // console.log(this.flags);
    let jestPath = path.join(binPath, "jest");
    return ws.spawn({ script: jestPath, opts: [] });
  }

  static register: Registration = program => program.command("test");
}
