import * as commander from "commander";
import path from "path";

import { Command } from "./command";
import { binPath, spawn } from "./common";

interface FmtFlags {}

export class FmtCommand extends Command {
  constructor(readonly flags: FmtFlags) {
    super();
  }

  async run(): Promise<boolean> {
    this.configManager.ensureConfig("prettier.config.js");
    let prettierBin = path.join(binPath, "prettier");
    let opts = ["-w", "src/**/*.{ts,tsx}"];
    return spawn(prettierBin, opts);
  }

  static register(program: commander.Command) {
    program.command("fmt").action(flags => new FmtCommand(flags).main());
  }
}
