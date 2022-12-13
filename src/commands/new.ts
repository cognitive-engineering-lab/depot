import { Option } from "commander";
import fs from "fs-extra";
import path from "path";
import sortPackageJson from "sort-package-json";

import { Registration, binPath, spawn } from "../common";
import { PLATFORMS, Platform, TARGETS, Target } from "../workspace";

interface NewFlags {
  name: string;
  target: Target;
  platform: Platform;
}

let INDEX = `import React from "react";
import ReactDOM from "react-dom/client";

let App = () => {
  return <h1>Hello world!</h1>;
};

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
`;

let HTML = `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/index.tsx"></script>
  </body>
</html>`;

let MAIN = `console.log("Hello world!");
`;

let LIB = ``;

export class NewCommand {
  constructor(readonly flags: NewFlags) {}

  async run() {
    let { name, target, platform } = this.flags;
    let dir = name.startsWith("@") ? name.split("/")[1] : name;
    await fs.mkdir(dir);
    await fs.mkdir(path.join(dir, "src"));

    let manifest: any = {
      name,
      version: "0.1.0",
    };

    let srcPath, srcContents;
    let devDependencies: string[] = [];
    if (target == "bin" && platform == "browser") {
      srcPath = "index.tsx";
      srcContents = INDEX;
      devDependencies = devDependencies.concat([
        "react",
        "react-dom",
        "@types/react",
        "@types/react-dom",
      ]);
      await fs.writeFile(path.join(dir, "index.html"), HTML);
    } else if (target == "bin" && platform == "node") {
      srcPath = "main.ts";
      srcContents = MAIN;
      manifest.bin = { [dir]: "dist/main.js" };
    } else {
      srcPath = "lib.ts";
      srcContents = LIB;
      manifest.main = "dist/lib.js";
      manifest.type = "module";
      manifest.files = ["dist"];
    }

    let gitignore = ["node_modules", "dist"].join("\n");

    let manifestPretty = sortPackageJson(
      JSON.stringify(manifest, undefined, 4)
    );
    await Promise.all([
      fs.writeFile(path.join(dir, "package.json"), manifestPretty),
      fs.writeFile(path.join(dir, "src", srcPath), srcContents),
      fs.writeFile(path.join(dir, ".gitignore"), gitignore),
    ]);

    if (devDependencies.length > 0) {
      let pnpmPath = path.join(binPath, "pnpm");
      await spawn({
        script: pnpmPath,
        opts: ["add", "-D", ...devDependencies],
        cwd: dir,
      });
    }

    await spawn({
      script: "graco",
      opts: ["init"],
      cwd: dir,
    });
  }

  static register: Registration = program =>
    program
      .command("new")
      .argument("<name>")
      .addOption(
        new Option("-t, --target <target>")
          .makeOptionMandatory()
          .choices(TARGETS)
      )
      .addOption(
        new Option("-p, --platform <platform>")
          .makeOptionMandatory()
          .choices(PLATFORMS)
      )
      .action((name, flags) => {
        console.log(name);
        new NewCommand({ name, ...flags }).run();
      });
}
