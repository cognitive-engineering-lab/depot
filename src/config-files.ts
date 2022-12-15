import fs from "fs-extra";
import path from "path";

import { gracoPkgRoot, symlinkExists } from "./common";
import { Package, Platform, Workspace } from "./workspace";

export const CONFIG_FILE_DIR = path.join(gracoPkgRoot, "dist", "assets");

export interface ConfigFile {
  name: string;
  path?: string;
  granularity: "workspace" | "package";
  platform?: Platform;
  monorepo?: boolean;
  copy?: boolean;
}

export let CONFIG_FILES: ConfigFile[] = [
  {
    name: "pnpm-workspace.yaml",
    granularity: "workspace",
    monorepo: true,
  },
  {
    name: ".eslintrc.cjs",
    granularity: "workspace",
  },
  {
    name: ".prettierrc.cjs",
    granularity: "workspace",
  },
  {
    name: "jest.config.cjs",
    path: "jest.config.workspace.cjs",
    granularity: "workspace",
    monorepo: true,
    copy: true,
  },
  {
    name: "jest.config.cjs",
    path: "jest.config.package.cjs",
    granularity: "package",
    copy: true,
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

export async function ensureConfig(config: ConfigFile, dir: string) {
  let srcPath = path.join(CONFIG_FILE_DIR, config.path || config.name);
  let dstPath = path.join(dir, config.name);
  if (await symlinkExists(dstPath)) return;
  if (config.copy) await fs.copyFile(srcPath, dstPath);
  else await fs.symlink(srcPath, dstPath);
  console.log(`Linked: ${dstPath}`);
}

export async function modifyGitignore(cfgs: ConfigFile[], dir: string) {
  let gitignorePath = path.join(dir, ".gitignore");
  if (!fs.existsSync(gitignorePath)) fs.createFileSync(gitignorePath);

  const HEADER = "# Managed by Graco";
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
        (!cfg.platform || cfg.platform == obj.platform) &&
        !(cfg.name == "jest.config.cjs" && !fs.existsSync(obj.path("tests")))
    );
  else
    return CONFIG_FILES.filter(
      cfg =>
        cfg.granularity == "workspace" &&
        (cfg.monorepo === undefined || obj.monorepo === cfg.monorepo)
    );
}
