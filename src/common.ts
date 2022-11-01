import * as commander from "commander";
import * as pty from "node-pty";
import path from "path";

import { Package, Workspace } from "./workspace";

export let modulesPath = path.resolve(
  path.join(__dirname, "..", "node_modules")
);

export let binPath = path.join(modulesPath, ".bin");

export interface SpawnProps {
  script: string;
  opts: string[];
  cwd: string;
  onData?: (data: string) => void;
}

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

export type Registration = (program: commander.Command) => commander.Command;
