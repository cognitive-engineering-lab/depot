import type * as commander from "commander";

import type { Command, CommonFlags, Registration } from "../common";
import { Workspace } from "../workspace";
import { BuildCommand } from "./build";
import { CleanCommand } from "./clean";
import { FmtCommand } from "./fmt";
import { InitCommand } from "./init";
import { NewCommand } from "./new";
import { TestCommand } from "./test";

export function registerCommands(program: commander.Command) {
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
  register(InitCommand, InitCommand.register);
  register(BuildCommand, BuildCommand.register);
  register(FmtCommand, FmtCommand.register);
  register(TestCommand, TestCommand.register);
  register(CleanCommand, CleanCommand.register);
}
