import fs from "fs-extra";
import * as pty from "node-pty";
import os from "os";
import type { IPackageJson } from "package-json-type";
import path from "path";

let BINPATH = path.join(__dirname, "..", "dist", "main.js");

export interface GracoProps {
  src: string | { [file: string]: string };
  manifest?: Partial<IPackageJson>;
  debug?: boolean;
}

export class Graco {
  constructor(readonly root: string, readonly debug: boolean) {}

  static async setup({ manifest, src, debug }: GracoProps): Promise<Graco> {
    let dir = await fs.mkdtemp(path.join(os.tmpdir(), "graco-test-"));
    if (debug) console.log(dir);

    let fullManifest: IPackageJson = {
      name: "test",
      ...(manifest || {}),
    };
    let p1 = fs.writeFile(
      path.join(dir, "package.json"),
      JSON.stringify(fullManifest)
    );

    let files = typeof src === "string" ? { "src/lib.ts": src } : src;

    let p2 = Promise.all(
      Object.keys(files).map(async f => {
        let fullPath = path.join(dir, f);
        await fs.mkdirp(path.dirname(fullPath));
        await fs.writeFile(fullPath, files[f]);
      })
    );

    await Promise.all([p1, p2]);

    let graco = new Graco(dir, debug || false);
    await graco.run("init");

    return graco;
  }

  run(cmd: string): Promise<number> {
    let p = pty.spawn(`node`, [BINPATH, cmd], { cwd: this.root });
    if (this.debug) p.onData(data => console.log(data));
    return new Promise(resolve =>
      p.onExit(({ exitCode }) => resolve(exitCode))
    );
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
