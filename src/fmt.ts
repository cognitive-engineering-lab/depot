import path from "path";

import { Command, Registration, binPath, spawn } from "./common";

interface FmtFlags {}

export class FmtCommand implements Command {
  constructor(readonly flags: FmtFlags) {}

  async run(): Promise<boolean> {
    let prettierBin = path.join(binPath, "prettier");
    let opts = ["-w", "src/**/*.{ts,tsx}"];
    return spawn(prettierBin, opts);
  }

  static register: Registration = program => program.command("fmt");
}
