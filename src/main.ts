import "@cspotcode/source-map-support/register.js";
import { program } from "commander";
import fs from "fs-extra";

import { BuildCommand } from "./build";
import { CleanCommand } from "./clean";
import type { Command, Registration } from "./common";
import { FmtCommand } from "./fmt";
import { InitCommand } from "./init";

type Class<T> = { new (...args: any[]): T };

function register<T extends Command>(Cmd: Class<T>, reg: Registration) {
  reg(program).action(async flags => {
    let cmd = new Cmd(flags);
    let exitCode = (await cmd.run()) ? 0 : 1;
    process.exit(exitCode);
  });
}

register(BuildCommand, BuildCommand.register);
register(FmtCommand, FmtCommand.register);
register(CleanCommand, CleanCommand.register);
register(InitCommand, InitCommand.register);

program.parseAsync(process.argv);
