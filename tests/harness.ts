import fs from "fs-extra";
import _ from "lodash";
import * as pty from "node-pty";
import os from "os";
import type { IPackageJson } from "package-json-type";
import path from "path";

let BINPATH = path.join(__dirname, "..", "dist", "main.mjs");

export interface GracoProps {
  src?: string | { [file: string]: string };
  manifest?: Partial<IPackageJson>;
  debug?: boolean;
  flags?: string;
}

function runGraco({
  cmd,
  cwd,
  debug,
}: {
  cmd: string;
  cwd: string;
  debug?: boolean;
}): Promise<number> {
  let p = pty.spawn("node", [BINPATH, ...cmd.split(" ")], { cwd });
  if (debug) p.onData(data => console.log(data));
  return new Promise(resolve => p.onExit(({ exitCode }) => resolve(exitCode)));
}

export class Graco {
  constructor(readonly root: string, readonly debug: boolean) {}

  static async setup({
    manifest,
    src,
    debug,
    flags,
  }: GracoProps): Promise<Graco> {
    let dir = await fs.mkdtemp(path.join(os.tmpdir(), "graco-test-"));
    if (debug) console.log(dir);

    flags = flags ?? "-t lib -p node";
    await runGraco({
      cmd: `new example ${flags}`,
      cwd: dir,
      debug,
    });
    let root = path.join(dir, "example");

    if (manifest) {
      let manifestPath = path.join(root, "package.json");
      let baseManifest = JSON.parse(await fs.readFile(manifestPath, "utf-8"));
      _.merge(baseManifest, manifest);
      await fs.writeFile(manifestPath, JSON.stringify(baseManifest));
    }

    if (src) {
      let files = typeof src === "string" ? { "src/lib.ts": src } : src;
      await Promise.all(
        Object.keys(files).map(async f => {
          let fullPath = path.join(root, f);
          await fs.mkdirp(path.dirname(fullPath));
          await fs.writeFile(fullPath, files[f]);
        })
      );
    }

    return new Graco(root, debug || false);
  }

  run(cmd: string): Promise<number> {
    return runGraco({ cmd, cwd: this.root, debug: this.debug });
  }

  read(file: string): string {
    return fs.readFileSync(path.join(this.root, file), "utf-8");
  }

  test(file: string) {
    if (!fs.existsSync(path.join(this.root, file))) {
      throw new Error(`File does not exist: ${file}`);
    }
  }

  async cleanup() {
    await fs.rm(this.root, { recursive: true });
  }

  static async with(props: GracoProps, f: (graco: Graco) => Promise<void>) {
    let graco = await Graco.setup(props);
    try {
      await f(graco);
    } finally {
      if (!props.debug) await graco.cleanup();
    }
  }
}
