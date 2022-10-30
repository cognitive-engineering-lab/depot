import fs from "fs-extra";
import path from "path";
import type { IPackageJson } from "package-json-type";
import * as cp from "child_process";
import os from "os";
import util from "util";

let exec = util.promisify(cp.exec);
let BINPATH = path.join(__dirname, "..", "dist", "main.js");

export interface GracoProps {
  src: string;
  manifest?: Partial<IPackageJson>;
  debug?: boolean;
}

export class Graco {
  constructor(readonly root: string) {}

  static async setup({ manifest, src, debug }: GracoProps): Promise<Graco> {
    let dir = await fs.mkdtemp(os.tmpdir());
    if (debug) console.log(dir);

    let fullManifest: IPackageJson = {
      name: "test",
      ...(manifest || {}),
    };
    let p1 = fs.writeFile(
      path.join(dir, "package.json"),
      JSON.stringify(fullManifest)
    );

    let srcDir = path.join(dir, "src");
    await fs.mkdir(srcDir);
    let p2 = fs.writeFile(path.join(srcDir, "lib.ts"), src);

    await Promise.all([p1, p2]);

    return new Graco(dir);
  }

  run(cmd: string): Promise<{ stderr: string; stdout: string }> {
    return exec(`node ${BINPATH} ${cmd}`, {
      cwd: this.root,
    });
  }

  read(file: string): string {
    return fs.readFileSync(path.join(this.root, file), "utf-8");
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
