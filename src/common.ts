import path from "path";
import fs from "fs-extra";
import type { IPackageJson } from "package-json-type";

export let modulesPath = path.resolve(
  path.join(__dirname, "..", "node_modules")
);

export let binPath = path.join(modulesPath, ".bin");

export let findJsFile = (basename: string): string | undefined => {
  let exts = ["tsx", "ts", "js"];
  return exts.map((e) => `${basename}.${e}`).find(fs.existsSync);
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
