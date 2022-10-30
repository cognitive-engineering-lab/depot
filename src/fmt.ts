import { Command } from "./command";
import * as commander from "commander";
import * as cp from "child_process";
import { binPath } from "./common";
import path from "path";

interface FmtFlags {}

export class FmtCommand extends Command {
  constructor(readonly flags: FmtFlags) {
    super();
  }

  async run(): Promise<boolean> {
    let prettierBin = path.join(binPath, "prettier");
    let opts = ["-w", "src/**/*.{ts,tsx}"];
    let prettier = cp.spawn(prettierBin, opts);
    prettier.stdout.pipe(process.stdout);
    prettier.stderr.pipe(process.stderr);
    return await new Promise((resolve) =>
      prettier.on("exit", (exitCode) => resolve(exitCode == 0))
    );
  }

  static register(program: commander.Command) {
    program.command("fmt").action((flags) => new FmtCommand(flags).main());
  }
}
