import fs from "fs-extra";
import * as pty from "node-pty";
import type { IPackageJson } from "package-json-type";
import path from "path";

export let modulesPath = path.resolve(
  path.join(__dirname, "..", "node_modules")
);

export let binPath = path.join(modulesPath, ".bin");

export let findJsFile = (basename: string): string | undefined => {
  let exts = ["tsx", "ts", "js"];
  return exts.map(e => `${basename}.${e}`).find(fs.existsSync);
};

/** Synchronously loads the current package's manifest (package.json) as a JS object.
 * Returns an empty object if package.json does not exist.
 */
export let getManifest = (): IPackageJson => {
  let pkgPath = "./package.json";
  return fs.existsSync(pkgPath)
    ? JSON.parse(fs.readFileSync("./package.json", "utf-8"))
    : {};
};

export let spawn = async (
  script: string,
  opts: string[],
  onData?: (data: string) => void
): Promise<boolean> => {
  let p = pty.spawn(script, opts, {
    env: {
      ...process.env,
      NODE_PATH: modulesPath,
    },
  });
  onData = onData || (data => process.stdout.write(data));
  p.onData(onData);
  let exitCode: number = await new Promise(resolve => {
    p.onExit(({ exitCode }) => resolve(exitCode));
  });
  return exitCode == 0;
};
