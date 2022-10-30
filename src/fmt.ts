import * as cp from "child_process";
import * as commander from "commander";
import path from "path";

import { Command } from "./command";
import { binPath } from "./common";

interface FmtFlags {}

export class FmtCommand extends Command {
  constructor(readonly flags: FmtFlags) {
    super();
  }

  async run(): Promise<boolean> {
    this.configManager.ensureConfig("prettier.config.js");
    let prettierBin = path.join(binPath, "prettier");
    let opts = ["-w", "src/**/*.{ts,tsx}"];
    let prettier = cp.spawn(prettierBin, opts);
    prettier.stdout.pipe(process.stdout);
    prettier.stderr.pipe(process.stderr);
    return await new Promise(resolve =>
      prettier.on("exit", exitCode => resolve(exitCode == 0))
    );
  }

  static register(program: commander.Command) {
    program.command("fmt").action(flags => new FmtCommand(flags).main());
  }
}
