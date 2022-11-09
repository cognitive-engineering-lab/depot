import * as commander from "commander";
import fs from "fs-extra";
import * as pty from "node-pty";
import path from "path";

import { Package, Workspace } from "./workspace";

declare global {
  var REPO_ROOT: string;
}

export let modulesPath = path.resolve(path.join(REPO_ROOT, "node_modules"));

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
  let p = pty.spawn(script, opts, {
    env: {
      ...process.env,
      NODE_PATH: modulesPath,
    },
    cwd,
  });
  onData = onData || (data => process.stdout.write(data));
  p.onData(onData);
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
