import fs from "fs-extra";

import { Command, Registration } from "./common";

interface CleanFlags {}

export class CleanCommand implements Command {
  constructor(readonly flags: CleanFlags) {}

  async run(): Promise<boolean> {
    let dirs = ["dist", "node_modules"];
    await Promise.all(
      dirs.map(d => fs.rm(d, { recursive: true, force: true }))
    );
    return true;
  }

  // todo: -a flag that also cleans up configs
  static register: Registration = program => program.command("clean");
}
