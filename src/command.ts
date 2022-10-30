import { ConfigManager } from "./config-files";

export abstract class Command {
  configManager: ConfigManager = new ConfigManager();

  abstract run(): Promise<boolean>;

  async main() {
    let exitCode;
    try {
      exitCode = (await this.run()) ? 0 : 1;
    } catch (e) {
      exitCode = 1;
    } finally {
      this.configManager.cleanup();
    }
    process.exit(exitCode);
  }
}
