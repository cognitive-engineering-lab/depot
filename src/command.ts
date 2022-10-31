export abstract class Command {
  abstract run(): Promise<boolean>;

  async main() {
    let exitCode = (await this.run()) ? 0 : 1;
    process.exit(exitCode);
  }
}
