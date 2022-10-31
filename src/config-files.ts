import fs from "fs-extra";
import path from "path";

type ConfigFile =
  | "tsconfig.json"
  | "vite.config.ts"
  | ".eslintrc.js"
  | "prettier.config.js"
  | "index.html";

export class ConfigManager {
  linked: Set<string>;
  constructor() {
    this.linked = new Set();
  }

  ensureConfig(file: ConfigFile) {
    if (!fs.existsSync(file)) {
      this.linked.add(file);
      fs.linkSync(path.join(__dirname, "assets", file), file);
    }
  }

  cleanup() {
    this.linked.forEach(file => {
      fs.rmSync(file);
    });
  }
}
