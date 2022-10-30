import fs from "fs-extra";
import path from "path";
import type { IPackageJson } from "package-json-type";
import * as cp from "child_process";
import os from "os";
import util from "util";

let exec = util.promisify(cp.exec);
let BINPATH = path.join(__dirname, "..", "dist", "main.js");

interface GracoProps {
  src: string;
  manifest?: Partial<IPackageJson>;
}

class Graco {
  constructor(readonly root: string) {}

  static async setup({ manifest, src }: GracoProps): Promise<Graco> {
    os;
    let dir = await fs.mkdtemp(os.tmpdir());

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

  async cleanup() {
    await fs.rm(this.root, { recursive: true });
  }

  static async with(props: GracoProps, f: (graco: Graco) => Promise<void>) {
    let graco = await Graco.setup(props);
    await f(graco);
    await graco.cleanup();
  }
}

test("build works", () =>
  Graco.with(
    {
      src: `
export let foo = "bar";  
`,
    },
    async (graco) => {
      await graco.run("build");
    }
  ));

test("tsc finds type errors", () =>
  Graco.with(
    {
      src: `
export let foo: number = "bar";  
`,
    },
    async (graco) => {
      await expect(graco.run("build")).rejects.toThrow();
    }
  ));
