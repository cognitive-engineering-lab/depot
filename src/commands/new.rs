use anyhow::{ensure, Context, Result};
use indexmap::indexmap;
use package_json_schema::PackageJson;
use serde_json::{json, Value};
#[cfg(unix)]
use std::os::unix::prelude::PermissionsExt;
use std::{
  borrow::Cow,
  env, fs,
  path::{Path, PathBuf},
};

use crate::workspace::{
  package::{PackageName, Platform, Target},
  Workspace,
};

use super::setup::GlobalConfig;

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

const TEST: &str = r#"import { add } from "../dist/lib";

test("add", () => expect(add(1, 2)).toBe(3));
"#;

const VITE_CONFIG: &str = include_str!("configs/vite.config.ts");
const PRETTIER_CONFIG: &str = include_str!("configs/.prettierrc.cjs");
const PNPM_WORKSPACE: &str = include_str!("configs/pnpm-workspace.yaml");

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
    let files: Vec<(PathBuf, Cow<'static, str>)> = vec![
      (
        "package.json".into(),
        serde_json::to_string_pretty(&manifest)?.into(),
      ),
      ("tsconfig.json".into(), self.make_tsconfig()?.into()),
      ("jest.config.cjs".into(), self.make_jest_config()?.into()),
      (".eslintrc.cjs".into(), self.make_eslint_config()?.into()),
      ("pnpm-workspace.yaml".into(), PNPM_WORKSPACE.into()),
      // (".prettierrc.cjs", self.make_p)
    ];

    for (rel_path, contents) in files {
      fs::write(root.join(rel_path), contents.as_bytes())?;
    }

    Ok(())
  }

  fn make_build_script(&self) -> String {
    let platform = self.args.platform.to_esbuild_string();
    format!(
      r#"import esbuild from "esbuild";

let watch = process.argv.includes("--watch");
let release = process.argv.includes("--release");

async function main() {{
  try {{
    let ctx = await esbuild.context({{
      entryPoints: ["src/main.ts"],
      platform: "{platform}",
      format: "esm",
      outdir: "dist",
      bundle: true,
      minify: release,
      sourcemap: !release,
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
    )
  }

  fn make_tsconfig(&self) -> Result<String> {
    let mut config = json!({
      "compilerOptions": {
        // Makes tsc respect "exports" directives in package.json
        "moduleResolution": "Node",

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

    Ok(serde_json::to_string_pretty(&config)?)
  }

  fn make_eslint_config(&self) -> Result<String> {
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
    Ok(format!("module.exports = {config_str}"))
  }

  fn make_jest_config(&self) -> Result<String> {
    let config = if !self.args.workspace {
      let test_environment = match self.args.platform {
        Platform::Browser => "jsdom",
        Platform::Node => "node",
      };
      json!({
        "preset": "ts-jest/presets/js-with-ts-esm",
        "roots": ["<rootDir>/tests"],
        "testEnvironment": test_environment
      })
    } else {
      json!({
        "projects": ["<rootDir>/packages/*"]
      })
    };
    let config_str = serde_json::to_string_pretty(&config)?;
    Ok(format!("module.exports = {config_str}"))
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

    let mut manifest = PackageJson::builder().build();
    manifest.name = Some(name.to_string());
    manifest.version = Some(String::from("0.1.0"));

    let mut files: Vec<(PathBuf, Cow<'static, str>)> = Vec::new();
    let mut dev_dependencies = Vec::new();
    let (src_path, src_contents) = match target {
      Target::Site => {
        ensure!(
          matches!(platform, Platform::Browser),
          "Cannot have platform=node when target=site"
        );
        dev_dependencies.extend(["react", "react-dom", "@types/react", "@types/react-dom"]);
        files.push(("index.html".into(), HTML.into()));
        files.push(("vite.config.ts".into(), VITE_CONFIG.into()));
        ("index.tsx", INDEX)
      }
      Target::Script => {
        if matches!(platform, Platform::Node) {
          manifest.bin = Some(package_json_schema::Binary::Object(indexmap! {
            name.name.clone() => String::from("dist/main.js")
          }));
        }
        files.push(("build.mjs".into(), self.make_build_script().into()));
        ("main.ts", MAIN)
      }
      Target::Lib => {
        manifest.main = Some(String::from("dist/lib.js"));
        manifest.type_ = Some(package_json_schema::Type::Module);
        manifest.files = Some(vec![String::from("dist")]);

        fs::create_dir(root.join("tests"))?;
        files.push(("tests/add.test.ts".into(), TEST.into()));

        ("lib.ts", LIB)
      }
    };

    let gitignore = ["node_modules", "dist"].join("\n");

    files.extend([
      (Path::new("src").join(src_path), src_contents.into()),
      (".gitignore".into(), gitignore.into()),
      (
        "package.json".into(),
        serde_json::to_string_pretty(&manifest)?.into(),
      ),
      ("tsconfig.json".into(), self.make_tsconfig()?.into()),
      (".eslintrc.cjs".into(), self.make_eslint_config()?.into()),
      ("jest.config.cjs".into(), self.make_jest_config()?.into()),
    ]);

    if self.ws_opt.is_none() {
      files.push((".prettierrc.cjs".into(), PRETTIER_CONFIG.into()));
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

    if !dev_dependencies.is_empty() {
      let mut pnpm = self.global_config.pnpm();
      pnpm
        .args(["add", "-D"])
        .args(dev_dependencies)
        .current_dir(root);
      let status = pnpm.status()?;
      ensure!(status.success(), "pnpm failed");
    }

    let _ws = match self.ws_opt.take() {
      Some(ws) => ws,
      None => Workspace::load(self.global_config, Some(root.to_owned())).await?,
    };

    // TODO: run workspace init

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
