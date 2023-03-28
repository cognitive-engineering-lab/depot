use anyhow::{ensure, Context, Result};
use indexmap::{indexmap, IndexMap};
use package_json_schema as pj;
use serde_json::{json, Value};
#[cfg(unix)]
use std::os::unix::prelude::PermissionsExt;
use std::{
  borrow::Cow,
  env,
  fs::{self, OpenOptions},
  io::{BufReader, Seek, Write},
  path::{Path, PathBuf},
  process::Command,
};

use crate::workspace::{
  package::{PackageGracoConfig, PackageName, Platform, Target},
  Workspace,
};

use super::{init::InitCommand, setup::GlobalConfig};

const INDEX: &str = r#"import React from "react";
import ReactDOM from "react-dom/client";

let App = () => {
  return <h1>Hello world!</h1>;
};

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
"#;

const HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/index.tsx"></script>
  </body>
</html>"#;

const MAIN: &str = r#"console.log("Hello world!");
"#;

const LIB: &str = r#"/** Adds two numbers together */
export function add(a: number, b: number) {
  return a + b;
}
"#;

const PRETTIER_CONFIG: &str = include_str!("configs/.prettierrc.cjs");
const PNPM_WORKSPACE: &str = include_str!("configs/pnpm-workspace.yaml");
const VITEST_SETUP: &str = include_str!("configs/setup.ts");

// TODO: option to specify --react that changes .ts -> .tsx

/// Create a new Graco workspace
#[derive(clap::Parser)]
pub struct NewArgs {
  name: PackageName,

  /// If a workspace should be created instead of a single package
  #[arg(short, long)]
  workspace: bool,

  /// Type of package
  #[arg(short, long, value_enum, default_value_t = Target::Lib)]
  target: Target,

  /// Where the package will run
  #[arg(short, long, value_enum, default_value_t = Platform::Browser)]
  platform: Platform,
}

pub struct NewCommand {
  args: NewArgs,
  ws_opt: Option<Workspace>,
  global_config: GlobalConfig,
}

fn json_merge(a: &mut Value, b: Value) {
  match (a, b) {
    (Value::Object(a), Value::Object(b)) => {
      for (k, b_v) in b {
        let a_v = a.entry(k).or_insert(Value::Null);
        json_merge(a_v, b_v);
      }
    }
    (Value::Array(a), Value::Array(b)) => {
      a.extend(b);
    }
    (a, b) => *a = b,
  };
}

type FileVec = Vec<(PathBuf, Cow<'static, str>)>;

impl NewCommand {
  pub async fn new(args: NewArgs, global_config: GlobalConfig) -> Self {
    let ws_opt = Workspace::load(global_config.clone(), None).await.ok();
    Self {
      args,
      ws_opt,
      global_config,
    }
  }

  fn new_workspace(self, root: &Path) -> Result<()> {
    fs::create_dir(root.join("packages"))?;

    let manifest = json!({"private": true});
    let mut files: FileVec = vec![
      (
        "package.json".into(),
        serde_json::to_string_pretty(&manifest)?.into(),
      ),
      ("pnpm-workspace.yaml".into(), PNPM_WORKSPACE.into()),
    ];
    files.extend(self.make_tsconfig()?);
    files.extend(self.make_eslint_config()?);
    files.extend(self.make_typedoc_config()?);
    files.extend(self.make_prettier_config());
    files.extend(self.make_gitignore());

    for (rel_path, contents) in files {
      fs::write(root.join(rel_path), contents.as_bytes())?;
    }

    Ok(())
  }

  fn make_build_script(&self) -> FileVec {
    let platform = match self.args.platform {
      Platform::Browser => "browser",
      Platform::Node => "node",
    };
    let src = format!(
      r#"import esbuild from "esbuild";
import fs from "fs";

let watch = process.argv.includes("--watch");
let release = process.argv.includes("--release");
let manifest = JSON.parse(fs.readFileSync("package.json", "utf-8"));

async function main() {{  
  try {{
    let external = Object.keys(manifest.dependencies || {{}});
    let ctx = await esbuild.context({{
      entryPoints: ["src/main.ts"],
      platform: "{platform}",
      format: "esm",
      outdir: "dist",
      bundle: true,
      minify: release,
      sourcemap: !release,
      external,
    }});
    if (watch) {{
      await ctx.watch();
    }} else {{
      await ctx.rebuild();
      await ctx.dispose();
    }}
  }} catch (e) {{
    console.error(e);
    process.exit(1);
  }}
}}


main();"#
    );
    vec![("build.mjs".into(), src.into())]
  }

  fn make_tsconfig(&self) -> Result<FileVec> {
    let mut config = json!({
      "compilerOptions": {
        // Makes tsc respect "exports" directives in package.json
        "moduleResolution": "bundler",

        // Makes tsc generate ESM syntax outputs
        "target": "es2022",

        // Generate .d.ts files for downstream consumers
        "declaration": true,

        // Allow JSX syntax in ts files
        "jsx": "react",

        // Allow ts-jest to import files from dist/ directory
        "esModuleInterop": true,
        "allowJs": true,
      },
    });

    if !self.args.workspace {
      if self.ws_opt.is_some() {
        config = json!({
          "extends": "../../tsconfig.json"
        });
      }

      json_merge(&mut config, json!({"include": ["src"]}));

      match self.args.target {
        Target::Lib => {
          json_merge(
            &mut config,
            json!({
              "compilerOptions": {
                "outDir": "dist"
              }
            }),
          );
        }
        Target::Script | Target::Site => {
          json_merge(
            &mut config,
            json!({
              "compilerOptions": {
                "noEmit": true
              }
            }),
          );
        }
      }
    }

    let src = serde_json::to_string_pretty(&config)?;
    Ok(vec![("tsconfig.json".into(), src.into())])
  }

  fn make_eslint_config(&self) -> Result<FileVec> {
    let mut config = json!({
      "env": {
        "es2021": true,
      },
      "extends": ["eslint:recommended"],
      "parser": "@typescript-eslint/parser",
      "parserOptions": {
        "ecmaVersion": 13,
        "sourceType": "module",
      },
      "plugins": ["@typescript-eslint", "prettier"],
      "ignorePatterns": ["*.d.ts"],
      "rules": {
        "no-empty-pattern": "off",
        "no-undef": "off",
        "no-unused-vars": "off",
        "no-cond-assign": "off",
        "@typescript-eslint/no-unused-vars": [
          "error",
          { "argsIgnorePattern": "^_", "varsIgnorePattern": "^_" },
        ],
        "no-constant-condition": ["error", { "checkLoops": false }],
        "prettier/prettier": "error",
      },
    });

    if !self.args.workspace && self.ws_opt.is_some() {
      config = json!({
        "extends": "../../.eslintrc.cjs"
      });

      let platform_config = match self.args.platform {
        Platform::Browser => json!({
          "env": {"browser": true},
          "plugins": ["react"],
          "rules": {
            "react/prop-types": "off",
            "react/no-unescaped-entities": "off",
          },
          "settings": {
            "react": {
              "version": "detect",
            },
          },
        }),
        Platform::Node => json!({
          "env": {
            "node": true,
          },
        }),
      };
      json_merge(&mut config, platform_config);
    }

    let config_str = serde_json::to_string_pretty(&config)?;
    let src = format!("module.exports = {config_str}");
    Ok(vec![(".eslintrc.cjs".into(), src.into())])
  }

  fn make_vite_config(&self) -> Result<FileVec> {
    let mut files: FileVec = Vec::new();
    let (environment, setup_files) = match self.args.platform {
      Platform::Browser => {
        files.push(("tests/setup.ts".into(), VITEST_SETUP.into()));
        ("jsdom", "\n  setupFiles: \"tests/setup.ts\",")
      }
      Platform::Node => ("node", ""),
    };

    // TODO: Revisit deps.inline once this issue is closed:
    // https://github.com/vitest-dev/vitest/issues/2806
    let test_config = format!(
      r#"test: {{
  environment: "{environment}",{setup_files}
  deps: {{
    inline: [/^(?!.*vitest).*$/],
  }},
}}"#
    );
    let test_config = textwrap::indent(&test_config, "  ");

    if matches!(self.args.target, Target::Site) {
      let src = format!(
        r#"import react from "@vitejs/plugin-react";
import {{ defineConfig }} from "vite";

export default defineConfig({{
  base: "./",
  plugins: [react()],
  build: {{ emptyOutDir: false }},
{test_config}
}});"#
      );
      files.push(("vite.config.ts".into(), src.into()));
    } else {
      let src = format!(
        r#"/// <reference types="vitest" />
import {{ defineConfig }} from "vite";

export default defineConfig({{
{test_config}
}});
"#
      );
      files.push(("vitest.config.ts".into(), src.into()));
    }

    Ok(files)
  }

  fn make_typedoc_config(&self) -> Result<FileVec> {
    let mut config = json!({
      "name": &self.args.name.name,
      "validation": {
        "invalidLink": true,
        "notExported": true
      }
    });

    if self.args.workspace {
      json_merge(
        &mut config,
        json!({
          "entryPointStrategy": "packages",
          "entryPoints": []
        }),
      );
    } else {
      json_merge(
        &mut config,
        json!({
          "entryPoints": ["src/lib.ts"]
        }),
      );
    }

    let src = serde_json::to_string_pretty(&config)?;
    Ok(vec![("typedoc.json".into(), src.into())])
  }

  fn update_typedoc_config(&self, ws: &Workspace) -> Result<()> {
    let mut f = OpenOptions::new()
      .read(true)
      .write(true)
      .open(ws.root.join("typedoc.json"))?;
    let mut config: Value = {
      let reader = BufReader::new(&mut f);
      serde_json::from_reader(reader)?
    };

    let entry_points = config
      .as_object_mut()
      .unwrap()
      .get_mut("entryPoints")
      .unwrap()
      .as_array_mut()
      .unwrap();
    entry_points.push(Value::String(format!("packages/{}", self.args.name.name)));

    f.rewind()?;
    let config_bytes = serde_json::to_vec_pretty(&config)?;
    f.write_all(&config_bytes)?;

    Ok(())
  }

  fn make_gitignore(&self) -> FileVec {
    let gitignore = ["node_modules", "dist", "docs"].join("\n");
    vec![(".gitignore".into(), gitignore.into())]
  }

  fn make_prettier_config(&self) -> FileVec {
    vec![(".prettierrc.cjs".into(), PRETTIER_CONFIG.into())]
  }

  async fn new_package(mut self, root: &Path) -> Result<()> {
    let NewArgs {
      name,
      target,
      platform,
      ..
    } = &self.args;

    let src_dir = root.join("src");
    fs::create_dir(&src_dir)
      .with_context(|| format!("Failed to create source directory: {}", src_dir.display()))?;

    let tests_dir = root.join("tests");
    fs::create_dir(&tests_dir)
      .with_context(|| format!("Failed to create tests directory: {}", tests_dir.display()))?;

    let mut manifest = pj::PackageJson::builder().build();
    manifest.name = Some(name.to_string());
    manifest.version = Some(String::from("0.1.0"));

    let mut files: FileVec = Vec::new();
    let mut ws_dependencies = Vec::new();
    let mut peer_dependencies = Vec::new();
    let mut other: IndexMap<String, Value> = IndexMap::new();

    let pkg_config = PackageGracoConfig {
      platform: *platform,
    };
    other.insert("graco".into(), serde_json::to_value(&pkg_config)?);

    match platform {
      Platform::Browser => {
        ws_dependencies.extend([
          "react",
          "react-dom",
          "@types/react",
          "@types/react-dom",
          "@testing-library/react",
        ]);
        if matches!(target, Target::Lib) {
          peer_dependencies.extend(["react", "react-dom"]);
        }
      }
      Platform::Node => {}
    }

    let (src_path, src_contents) = match target {
      Target::Site => {
        ensure!(
          matches!(platform, Platform::Browser),
          "Must have platform=browser when target=site"
        );
        files.push(("index.html".into(), HTML.into()));
        ("index.tsx", INDEX)
      }
      Target::Script => {
        if matches!(platform, Platform::Node) {
          manifest.bin = Some(pj::Binary::Object(indexmap! {
            name.name.clone() => "dist/main.js".into()
          }));
        }
        files.extend(self.make_build_script());
        ("main.ts", MAIN)
      }
      Target::Lib => {
        manifest.main = Some(String::from("dist/lib.js"));
        manifest.type_ = Some(pj::Type::Module);
        manifest.files = Some(vec![String::from("dist")]);
        let main_export = pj::ExportsObject::builder()
          .default("./dist/lib.js")
          .build();
        let sub_exports = pj::ExportsObject::builder().default("./dist/*.js").build();
        manifest.exports = Some(pj::Exports::Nested(indexmap! {
          ".".into() => main_export,
          "./*".into() => sub_exports,
        }));

        let test = r#"import { test, expect } from "vitest";

import { add } from "../src/lib";

test("add", () => expect(add(2, 2)).toBe(4));
"#;

        files.push(("tests/add.test.ts".into(), test.into()));

        other.insert("typedoc".into(), json!({"entryPoint": "./src/lib.ts"}));

        match &self.ws_opt {
          Some(ws) => self.update_typedoc_config(ws)?,
          None => files.extend(self.make_typedoc_config()?),
        }

        ("lib.ts", LIB)
      }
    };

    manifest.other = Some(other);

    files.extend([
      (Path::new("src").join(src_path), src_contents.into()),
      (
        "package.json".into(),
        serde_json::to_string_pretty(&manifest)?.into(),
      ),
    ]);
    files.extend(self.make_tsconfig()?);
    files.extend(self.make_eslint_config()?);
    files.extend(self.make_vite_config()?);

    if self.ws_opt.is_none() {
      files.extend(self.make_gitignore());
      files.extend(self.make_prettier_config());
    }

    for (rel_path, contents) in files {
      fs::write(root.join(rel_path), contents.as_bytes())?;
    }

    #[cfg(unix)]
    {
      let build_script_path = root.join("build.mjs");
      if build_script_path.exists() {
        fs::set_permissions(build_script_path, PermissionsExt::from_mode(0o755))?;
      }
    }

    let pnpm_cmd = || Command::new(self.global_config.bindir().join("pnpm"));
    if !peer_dependencies.is_empty() {
      let mut pnpm = pnpm_cmd();
      pnpm
        .args(["add", "--save-peer"])
        .args(peer_dependencies)
        .current_dir(root);
      let status = pnpm.status()?;
      ensure!(status.success(), "pnpm failed");
    }

    if !ws_dependencies.is_empty() {
      let mut pnpm = pnpm_cmd();
      pnpm.args(["add", "--save-dev"]).args(ws_dependencies);

      match &self.ws_opt {
        Some(ws) => {
          pnpm.arg("--workspace-root").current_dir(&ws.root);
        }
        None => {
          pnpm.current_dir(root);
        }
      }

      let status = pnpm.status()?;
      ensure!(status.success(), "pnpm failed");
    }

    let ws = match self.ws_opt.take() {
      Some(ws) => ws,
      None => Workspace::load(self.global_config, Some(root.to_owned())).await?,
    };

    let cmd = InitCommand::new(Default::default());
    ws.run_both(&cmd).await?;

    Ok(())
  }

  pub async fn run(self) -> Result<()> {
    ensure!(
      !(self.ws_opt.is_some() && self.args.workspace),
      "Cannot create a new workspace inside an existing workspace"
    );

    let name = &self.args.name;
    let parent_dir = match &self.ws_opt {
      Some(ws) => ws.root.join("packages"),
      None => env::current_dir()?,
    };
    let root = parent_dir.join(&name.name);
    fs::create_dir(&root)
      .with_context(|| format!("Failed to create root directory: {}", root.display()))?;

    if self.args.workspace {
      self.new_workspace(&root)
    } else {
      self.new_package(&root).await
    }
  }
}
