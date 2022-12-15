import * as cp from "child_process";
import * as commander from "commander";
import fs from "fs-extra";
import * as pty from "node-pty";
import path from "path";
import { fileURLToPath } from "url";

import { log } from "./log";
import { Package, Workspace } from "./workspace";

let thisDir = path.resolve(
  path.join(path.dirname(fileURLToPath(import.meta.url)), "..")
);
let findPkgRoot = () => {
  let globalRoot = cp.execSync("npm root -g", { encoding: "utf-8" });
  return path.join(globalRoot, "graco");
};
export let gracoPkgRoot = fs.existsSync(path.join(thisDir, "node_modules"))
  ? thisDir
  : findPkgRoot();

export let modulesPath = path.join(gracoPkgRoot, "node_modules");

export let binPath = path.join(modulesPath, ".bin");

export interface SpawnProps {
  script: string;
  opts: string[];
  cwd: string;
  onData?: (data: string) => void;
}

export let symlinkExists = async (file: string): Promise<boolean> => {
  try {
    let lstat = await fs.lstat(file);
    return lstat.isFile() || lstat.isSymbolicLink();
  } catch (e) {
    return false;
  }
};

export let spawn = async ({
  script,
  opts,
  cwd,
  onData,
}: SpawnProps): Promise<boolean> => {
  let p: pty.IPty;
  try {
    p = pty.spawn(script, opts, {
      env: {
        ...process.env,
        NODE_PATH: modulesPath,
      },
      cwd,
    });
  } catch (e) {
    log.error(
      `Failed to spawn process: ${script}\nGraco package root is: ${gracoPkgRoot}`
    );
    return false;
  }

  onData = onData || (data => process.stdout.write(data));
  p.onData(onData);
  ["SIGINT", "SIGTERM"].forEach(signal => process.on(signal, () => p.kill()));
  let exitCode: number = await new Promise(resolve => {
    p.onExit(({ exitCode }) => resolve(exitCode));
  });
  return exitCode == 0;
};

export interface CommonOpts {
  workspace: boolean;
}

export interface Command {
  parallel?(): boolean;
  run?(pkg: Package): Promise<boolean>;
  runWorkspace?(ws: Workspace): Promise<boolean>;
}

export interface CommonFlags {
  packages?: string[];
}

export type Registration = (program: commander.Command) => commander.Command;
