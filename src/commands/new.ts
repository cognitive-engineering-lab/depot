import { Option } from "commander";
import fs from "fs-extra";
import _ from "lodash";
import path from "path";
import sortPackageJson from "sort-package-json";

import PRETTIER_CONFIG from "../assets/.prettierrc.cjs?raw";
import PNPM_WORKSPACE from "../assets/pnpm-workspace.yaml?raw";
import VITE_CONFIG from "../assets/vite.config.ts?raw";
import { Registration, binPath, spawn } from "../common";
import { PLATFORMS, Platform, TARGETS, Target, Workspace } from "../workspace";
import { InitCommand } from "./init";

interface NewFlags {
  name: string;
  target: Target;
  platform: Platform;
  workspace?: boolean;
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

let LIB = `/** Adds two numbers together */
export function add(a: number, b: number) {
  return a + b;
}
`;

function merge(dst: any, src: any) {
  _.mergeWith(dst, src, (obj, src) =>
    _.isArray(obj) ? obj.concat(src) : undefined
  );
}

type ConfigKind = "package" | "workspace";

export class NewCommand {
  ws?: Workspace;
  constructor(readonly flags: NewFlags) {}

  async run() {
    try {
      this.ws = await Workspace.load();
    } catch (exc: any) {
      // Not in a workspace
      this.ws = undefined;
    }

    let { name } = this.flags;
    let dir = name.startsWith("@") ? name.split("/")[1] : name;
    let root =
      this.ws && !this.flags.workspace
        ? path.join(this.ws.root, "packages")
        : process.cwd();
    let absDir = path.join(root, dir);
    await fs.mkdir(absDir);

    if (this.flags.workspace) this.newWorkspace(absDir);
    else this.newPackage(absDir);
  }

  async newWorkspace(dir: string) {
    await fs.mkdir(path.join(dir, "packages"));
    let manifest = {
      private: true,
    };
    let files = [
      ["package.json", this.manifestToString(manifest)],
      ["tsconfig.json", this.makeTsconfig("workspace")],
      ["jest.config.cjs", this.makeJestConfig("workspace")],
      [".eslintrc.cjs", this.makeEslintConfig("workspace")],
      [".prettierrc.cjs", PRETTIER_CONFIG],
      ["pnpm-workspace.yaml", PNPM_WORKSPACE],
    ];
    await Promise.all(
      files.map(([name, contents]) =>
        fs.writeFile(path.join(dir, name), contents)
      )
    );
  }

  makeTsconfig(kind: ConfigKind) {
    let baseConfig = {
      compilerOptions: {
        // Makes tsc respect "exports" directives in package.json
        moduleResolution: "Node16",

        // Makes tsc generate ESM syntax outputs
        target: "es2022",

        // Generate .d.ts files for downstream consumers
        declaration: true,

        // Allow JSX syntax in ts files
        jsx: "react",

        // Allow ts-jest to import files from dist/ directory
        esModuleInterop: true,
        allowJs: true,
      },
    };

    let config: any;
    if (kind == "package") {
      config = this.ws
        ? {
            extends: "../../tsconfig.json",
          }
        : baseConfig;

      merge(config, { include: ["src"] });

      if (this.flags.target == "lib") {
        merge(config, {
          compilerOptions: {
            outDir: "dist",
          },
        });
      } else {
        merge(config, {
          compilerOptions: {
            noEmit: true,
          },
        });
      }
    } else {
      config = baseConfig;
    }

    return JSON.stringify(config, undefined, 4);
  }

  makeEslintConfig(kind: ConfigKind) {
    let baseConfig = {
      env: {
        es2021: true,
      },
      extends: ["eslint:recommended"],
      parser: "@typescript-eslint/parser",
      parserOptions: {
        ecmaVersion: 13,
        sourceType: "module",
      },
      plugins: ["@typescript-eslint", "prettier"],
      ignorePatterns: ["*.d.ts"],
      rules: {
        "no-empty-pattern": "off",
        "no-undef": "off",
        "no-unused-vars": "off",
        "no-cond-assign": "off",
        "@typescript-eslint/no-unused-vars": [
          "error",
          { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
        ],
        "no-constant-condition": ["error", { checkLoops: false }],
        "prettier/prettier": "error",
      },
    };

    let config: any;
    if (kind == "package") {
      config = this.ws
        ? {
            extends: ["../../.eslintrc.cjs"],
          }
        : baseConfig;

      let platformConfig;
      if (this.flags.platform == "browser") {
        platformConfig = {
          env: {
            browser: true,
          },
          plugins: ["react"],
          rules: {
            "react/prop-types": "off",
            "react/no-unescaped-entities": "off",
          },
          settings: {
            react: {
              version: "detect",
            },
          },
        };
      } else {
        platformConfig = {
          env: {
            node: true,
          },
        };
      }
      merge(config, platformConfig);
    } else {
      config = baseConfig;
    }

    return `module.exports = ${JSON.stringify(config, undefined, 4)}`;
  }

  makeJestConfig(kind: ConfigKind) {
    let config: any;
    if (kind == "package") {
      config = {
        preset: "ts-jest/presets/js-with-ts-esm",
        roots: ["<rootDir>/tests"],
      };

      if (this.flags.platform == "browser") config.testEnvironment = "jsdom";
      else config.testEnvironment = "node";
    } else {
      config = {
        projects: ["<rootDir>/packages/*"],
      };
    }

    return `module.exports = ${JSON.stringify(config, undefined, 4)}`;
  }

  manifestToString(manifest: any): string {
    return sortPackageJson(JSON.stringify(manifest, undefined, 4));
  }

  async newPackage(dir: string) {
    let { name, target, platform } = this.flags;

    if (!target) throw new Error("Must provide a target with -t");
    if (!platform) throw new Error("Must provide a platform with -p");

    await fs.mkdir(path.join(dir, "src"));

    let manifest: any = {
      name,
      version: "0.1.0",
    };

    let files: [string, string][] = [];
    let srcPath, srcContents;
    let devDependencies: string[] = [];

    if (target == "site") {
      if (platform == "node")
        throw new Error(`Cannot have platform=node when target=site`);
      srcPath = "index.tsx";
      srcContents = INDEX;

      devDependencies.push(
        "react",
        "react-dom",
        "@types/react",
        "@types/react-dom"
      );

      files.push(["index.html", HTML]);
      files.push(["vite.config.ts", VITE_CONFIG]);
    } else if (target == "bin") {
      srcPath = "main.ts";
      srcContents = MAIN;

      manifest.bin = { [dir]: "dist/main.js" };
    } /* (target == "lib") */ else {
      srcPath = "lib.ts";
      srcContents = LIB;

      manifest.main = "dist/lib.js";
      manifest.type = "module";
      manifest.files = ["dist"];
      manifest.exports = {
        ".": "./dist/lib.js",
        "./*": "./dist/*.js",
      };

      await fs.mkdir(path.join(dir, "tests"));
      let sampleTest = `import {add} from "${name}";
test("add", () => expect(add(1, 2)).toBe(3));`;
      files.push(["tests/add.test.ts", sampleTest]);
    }

    let gitignore = ["node_modules", "dist"].join("\n");

    files.push(
      [path.join("src", srcPath), srcContents],
      [".gitignore", gitignore],
      ["package.json", this.manifestToString(manifest)],
      ["tsconfig.json", this.makeTsconfig("package")],
      [".eslintrc.cjs", this.makeEslintConfig("package")],
      ["jest.config.cjs", this.makeJestConfig("package")]
    );

    if (!this.ws) files.push([".prettierrc.cjs", PRETTIER_CONFIG]);

    await Promise.all(
      files.map(([name, contents]) =>
        fs.writeFile(path.join(dir, name), contents)
      )
    );

    if (devDependencies.length > 0) {
      let pnpmPath = path.join(binPath, "pnpm");
      await spawn({
        script: pnpmPath,
        opts: ["add", "-D", ...devDependencies],
        cwd: dir,
      });
    }

    let ws = await Workspace.load(dir);
    ws.run(new InitCommand({}));
  }

  static register: Registration = program =>
    program
      .command("new")
      .description("Create a new Graco workspace")
      .argument("<name>")
      .addOption(
        new Option("-t, --target <target>", "Type of package")
          .choices(TARGETS)
          .default("lib")
      )
      .addOption(
        new Option("-p, --platform <platform>", "Where the package will run")
          .choices(PLATFORMS)
          .default("browser")
      )
      .option(
        "-w, --workspace",
        "If a workspace should be created instead of a single package"
      )
      .action((name, flags) => {
        new NewCommand({ name, ...flags }).run();
      });
}
