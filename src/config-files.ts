import fs from "fs-extra";
import path from "path";

import { Package, Platform, Workspace } from "./workspace";

export const CONFIG_FILE_DIR = path.join(__dirname, "assets");

export interface ConfigFile {
  name: string;
  granularity: "workspace" | "package";
  platform?: Platform;
  monorepo?: boolean;
}

export let CONFIG_FILES: ConfigFile[] = [
  {
    name: "pnpm-workspace.yaml",
    granularity: "workspace",
    monorepo: true,
  },
  {
    name: ".eslintrc.js",
    granularity: "workspace",
  },
  {
    name: ".prettierrc.js",
    granularity: "workspace",
  },
  {
    name: "jest.config.js",
    granularity: "workspace",
  },
  {
    name: "vite.config.ts",
    granularity: "package",
    platform: "browser",
  },
  {
    name: "tsconfig.json",
    granularity: "package",
  },
];

export async function modifyGitignore(cfgs: ConfigFile[], dir: string) {
  let gitignorePath = path.join(dir, ".gitignore");
  if (!fs.existsSync(gitignorePath)) fs.createFileSync(gitignorePath);

  const HEADER = "# Managed by Greco";
  let contents = await fs.readFile(gitignorePath, "utf-8");
  let entries = contents.split("\n");
  let i = entries.indexOf(HEADER);
  if (i == -1) i = entries.length;

  let toIgnore = await findManagedConfigs(cfgs, dir);
  toIgnore.sort();
  let newEntries = entries
    .slice(0, i)
    .concat([HEADER, ...toIgnore.map(cfg => cfg.name)]);
  await fs.writeFile(gitignorePath, newEntries.join("\n"));
}

export async function findManagedConfigs(
  cfgs: ConfigFile[],
  dir: string
): Promise<ConfigFile[]> {
  let isDefault = await Promise.all(
    cfgs.map(async config => {
      let fullPath = path.join(dir, config.name);
      try {
        let p = await fs.realpath(fullPath);
        return path.dirname(p) == CONFIG_FILE_DIR;
      } catch (e) {
        return false;
      }
    })
  );
  return cfgs.filter((_f, i) => isDefault[i]);
}

export function configsFor(obj: Package | Workspace): ConfigFile[] {
  if (obj instanceof Package)
    return CONFIG_FILES.filter(
      cfg =>
        cfg.granularity == "package" &&
        (!cfg.platform || cfg.platform == obj.platform)
    );
  else
    return CONFIG_FILES.filter(
      cfg => cfg.granularity == "workspace" && (obj.monorepo || !cfg.monorepo)
    );
}
