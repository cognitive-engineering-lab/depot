import "@cspotcode/source-map-support/register.js";
import { program } from "commander";

import { BuildCommand } from "./build";
import { CleanCommand } from "./clean";
import type { Command, CommonFlags, Registration } from "./common";
import { FmtCommand } from "./fmt";
import { InitCommand } from "./init";
import { NewCommand } from "./new";
import { TestCommand } from "./test";
import { Workspace } from "./workspace";

type Class<T> = { new (...args: any[]): T };
function register<T extends Command>(Cmd: Class<T>, reg: Registration) {
  reg(program)
    .option("-p, --packages <packages...>")
    .action(async (flags: CommonFlags) => {
      let ws = await Workspace.load();
      let success = await ws.run(new Cmd(flags, ws), flags.packages);
      process.exit(success ? 0 : 1);
    });
}

NewCommand.register(program);
register(BuildCommand, BuildCommand.register);
register(FmtCommand, FmtCommand.register);
register(CleanCommand, CleanCommand.register);
register(InitCommand, InitCommand.register);
register(TestCommand, TestCommand.register);

program.parseAsync(process.argv);
