import path from "path";

import { Command, Registration, binPath, spawn } from "../common";
import { Package } from "../workspace";

interface FmtFlags {}

export class FmtCommand implements Command {
  constructor(readonly flags: FmtFlags) {}

  async run(pkg: Package): Promise<boolean> {
    let prettierBin = path.join(binPath, "prettier");
    let opts = ["-w", "{src,tests}/**/*.{ts,tsx}"];
    return pkg.spawn({ script: prettierBin, opts });
  }

  parallel() {
    return true;
  }

  static register: Registration = program =>
    program.command("fmt").description("Auto-format source files");
}
