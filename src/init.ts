import { Command, Registration, spawn } from "./common";
import { ConfigManager } from "./config-files";

interface InitFlags {}

export class InitCommand implements Command {
  constructor(readonly flags: InitFlags) {}

  async run(): Promise<boolean> {
    let configManager = new ConfigManager();
    configManager.ensureAllConfigsExist();

    return spawn("pnpm", ["install"]);
  }

  static register: Registration = program => program.command("init");
}
