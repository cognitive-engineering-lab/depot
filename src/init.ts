import * as commander from "commander";
import path from "path";
import fs from "fs-extra";

import { Command } from "./command";
import { binPath, findJsFile, spawn } from "./common";
import { ConfigManager } from "./config-files";

interface InitFlags {}

export class InitCommand extends Command {
  constructor(readonly flags: InitFlags) {
    super();
  }

  async run(): Promise<boolean> {
    let configManager = new ConfigManager();
    configManager.ensureAllConfigsExist();

    return spawn("pnpm", ["install"]);
  }

  static register(program: commander.Command) {
    program.command("init").action((flags) => new InitCommand(flags).main());
  }
}
