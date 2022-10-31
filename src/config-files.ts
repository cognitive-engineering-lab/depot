import fs from "fs-extra";
import path from "path";

import { findJsFile } from "./common";

const ASSETS_DIR = path.join(__dirname, "assets");

export class ConfigManager {
  configs: string[];

  constructor() {
    this.configs = ["tsconfig.json", ".eslintrc.js", ".prettierrc.js"];

    let index = findJsFile("src/index");
    if (index) {
      this.configs.push("vite.config.ts");
      this.configs.push("index.html");
    }

    // if (fs.existsSync("packages")) {
    //   this.configs.push("pnpm-workspace.yaml");
    // }
  }

  async modifyGitignore() {
    if (!fs.existsSync(".gitignore")) fs.createFileSync(".gitignore");

    const HEADER = "# Managed by Greco";
    let contents = await fs.readFile(".gitignore", "utf-8");
    let entries = contents.split("\n");
    let i = entries.indexOf(HEADER);
    if (i == -1) i = entries.length;

    let toIgnore = await this.findDefaultConfigs();
    toIgnore.sort();
    let newEntries = entries.slice(0, i).concat([HEADER, ...toIgnore]);
    await fs.writeFile(".gitignore", newEntries.join("\n"));
  }

  async findDefaultConfigs() {
    let isDefault = await Promise.all(
      this.configs.map(async config => {
        let p = await fs.realpath(config);
        return path.dirname(p) == ASSETS_DIR;
      })
    );
    return this.configs.filter((_f, i) => isDefault[i]);
  }

  async ensureAllConfigsExist() {
    let promises = this.configs.map(async config => {
      if (fs.existsSync(config)) return;
      await fs.symlink(path.join(ASSETS_DIR, config), config);
    });
    await Promise.all(promises);

    await this.modifyGitignore();
  }
}
