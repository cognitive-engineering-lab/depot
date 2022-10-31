import * as commander from "commander";
import fs from "fs-extra";

import { Command } from "./command";

interface CleanFlags {}

export class CleanCommand extends Command {
  constructor(readonly flags: CleanFlags) {
    super();
  }

  async run(): Promise<boolean> {
    let dirs = ["dist", "node_modules"];
    await Promise.all(
      dirs.map(d => fs.rm(d, { recursive: true, force: true }))
    );
    return true;
  }

  static register(program: commander.Command) {
    program.command("clean").action(flags => new CleanCommand(flags).main());
  }
}
