#![allow(clippy::items_after_statements, clippy::too_many_lines)]

use anyhow::{ensure, Context, Result};
use indexmap::{indexmap, IndexMap};
use package_json_schema as pj;
use serde_json::{json, Value};
use std::{
  borrow::Cow,
  env,
  fs::OpenOptions,
  io::{BufReader, Seek, Write},
  path::{Path, PathBuf},
  process::Command,
  str::FromStr,
};

use crate::{
  utils,
  workspace::{
    package::{PackageDepotConfig, PackageName, Platform, Target},
    Workspace, WorkspaceDepotConfig, DEPOT_VERSION,
  },
  CommonArgs,
};

use super::setup::GlobalConfig;

const REACT_INDEX: &str = r#"import React from "react";
import ReactDOM from "react-dom/client";

let App = () => {
  return <h1>Hello world!</h1>;
};

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
"#;

const BASIC_INDEX: &str = r#"let root = document.getElementById("root")!;
root.innerHTML = "<h1>Hello world!</h1>";
"#;

const MAIN: &str = r#"console.log("Hello world!");
"#;

const LIB: &str = "/** Adds two numbers together */
export function add(a: number, b: number) {
  return a + b;
}
";

const TEST: &str = r#"import { expect, test } from "vitest";

import { add } from "../src/lib";

test("add", () => expect(add(2, 2)).toBe(4));
"#;

const CSS: &str = r#"@import "normalize.css/normalize.css";
"#;

const PNPM_WORKSPACE: &str = include_str!("configs/pnpm-workspace.yaml");
const VITEST_SETUP: &str = include_str!("configs/setup.ts");

/// Create a new Depot workspace
#[derive(clap::Parser)]
#[allow(clippy::struct_excessive_bools)]
pub struct NewArgs {
  pub name: PackageName,

  /// If a workspace should be created instead of a single package
  #[arg(short, long)]
  pub workspace: bool,

  /// Type of package
  #[arg(short, long, value_enum, default_value_t = Target::Lib)]
  pub target: Target,

  /// Where the package will run
  #[arg(short, long, value_enum, default_value_t = Platform::Browser)]
  pub platform: Platform,

  /// Add React as a project dependency
  #[arg(long, action)]
  pub react: bool,

  /// Add Vike as a project dependency
  #[arg(long, action)]
  pub vike: bool,

  /// Add Sass as a project dependency
  #[arg(long, action)]
  pub sass: bool,

  /// Don't attempt to download packages from the web
  #[arg(long, action)]
  pub offline: bool,

  /// Prefer local pnpm cache if available
  #[arg(long, action)]
  pub prefer_offline: bool,
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

#[test]
fn test_json_merge() {
  let mut a = json!({
    "a": 0,
    "b": {
      "c": [1],
      "d": 2
    }
  });
  json_merge(
    &mut a,
    json!({
      "e": 3,
      "b": {
        "c": [4],
        "f": 5
      }
    }),
  );
  assert_eq!(
    a,
    json!({
      "a": 0,
      "b": {
        "c": [1, 4],
        "d": 2,
        "f": 5
      },
      "e": 3
    })
  );
}

type FileVec = Vec<(PathBuf, Cow<'static, str>)>;

impl NewCommand {
  pub async fn new(args: NewArgs, global_config: GlobalConfig) -> Self {
    let ws_opt = Workspace::load(global_config.clone(), None, CommonArgs::default())
      .await
      .ok();
    Self {
      args,
      ws_opt,
      global_config,
    }
  }

  fn new_workspace(self, root: &Path) -> Result<()> {
    utils::create_dir(root.join("packages"))?;

    let manifest = json!({
      "private": true,
      "depot": {
        "depot-version": DEPOT_VERSION
      },
      // STUPID HACK: see note on same code in new_package
      "pnpm": {
        "overrides": {
          "rollup": "npm:@rollup/wasm-node"
        }
      }
    });
    let mut files: FileVec = vec![
      (
        "package.json".into(),
        serde_json::to_string_pretty(&manifest)?.into(),
      ),
      ("pnpm-workspace.yaml".into(), PNPM_WORKSPACE.into()),
    ];
    files.extend(self.make_tsconfig()?);
    files.extend(self.make_biome_config()?);
    files.extend(self.make_typedoc_config()?);
    files.extend(Self::make_gitignore());

    for (rel_path, contents) in files {
      utils::write(root.join(rel_path), contents.as_bytes())?;
    }

    self.install_ws_dependencies(root, true)?;

    Ok(())
  }

  fn make_tsconfig(&self) -> Result<FileVec> {
    let mut files: FileVec = Vec::new();
    let mut config = json!({
      "compilerOptions": {
        // Makes tsc respect "exports" directives in package.json
        "moduleResolution": "bundler",

        // Makes tsc generate ESM syntax outputs
        "target": "es2022",

        // Generate .d.ts files for downstream consumers
        "declaration": true,

        // Allow JS files to be included
        "allowJs": true,

        // Prevent tsc from checking files in node_modules
        // See: https://stackoverflow.com/a/57653497
        // TODO: pretty sure this is not ideal... need to figure out
        //   a better fix
        "skipLibCheck": true,

        // Enables several useful static checks
        // See: https://www.typescriptlang.org/tsconfig#strict
        "strict": true,
      },
    });

    if self.args.react {
      json_merge(
        &mut config,
        json!({
          "compilerOptions": {
            // Allow JSX syntax in ts files
            "jsx": "react",
          }
        }),
      );
    }

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
              },
              "typedocOptions": {
                "entryPoints": ["./src/lib.ts"]
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

      if self.args.platform.is_browser() {
        // Allows special Vite things like importing files with ?raw
        files.push((
          "src/bindings/vite.d.ts".into(),
          r#"/// <reference types="vite/client" />
"#
          .into(),
        ));
      }
    }

    let src = serde_json::to_string_pretty(&config)?;
    files.push(("tsconfig.json".into(), src.into()));
    Ok(files)
  }

  fn make_biome_config(&self) -> Result<FileVec> {
    let mut config = json!({
      "$schema": "https://biomejs.dev/schemas/1.8.2/schema.json",
      "javascript": {
        "formatter": {
          "arrowParentheses": "asNeeded",
          "trailingCommas": "none"
        }
      },
      "formatter": {
        "indentStyle": "space"
      },
      "linter": {
        "rules": {
          "recommended": true,
          "correctness": {"noUnusedImports": "warn"},
          "style": {"noNonNullAssertion": "off", "useConst": "off", "noUselessElse": "off"},
          "complexity": { "noBannedTypes": "off", "noForEach": "off" },
        }
      }
    });

    if self.args.react {
      json_merge(
        &mut config,
        json!({
          "javascript": {
            "jsxRuntime": "reactClassic",
          },
          "linter": {
            "rules": {
              "correctness": {"useExhaustiveDependencies": "off", "useJsxKeyInIterable": "off"},
              "suspicious": {"noArrayIndexKey": "off"}
            }
          }
        }),
      );
    }

    let config_str = serde_json::to_string_pretty(&config)?;
    Ok(vec![("biome.json".into(), config_str.into())])
  }

  fn make_vite_config(&self, entry_point: Option<&str>) -> FileVec {
    let NewArgs {
      platform, target, ..
    } = self.args;

    let mut files: FileVec = Vec::new();
    let environment = match platform {
      Platform::Browser => "jsdom",
      Platform::Node => "node",
    };

    let setup_files = if self.args.react {
      files.push(("tests/setup.ts".into(), VITEST_SETUP.into()));
      "\n  setupFiles: \"tests/setup.ts\","
    } else {
      ""
    };

    let mut imports = vec![("fs", "node:fs")];
    if self.args.react {
      imports.push(("react", "@vitejs/plugin-react"));
    }
    if self.args.vike {
      imports.push(("vike", "vike/plugin"));
    }
    imports.push(("{ defineConfig }", "vite"));

    let mut config: Vec<(&str, Cow<'static, str>)> = Vec::new();

    match target {
      Target::Site => {
        if !self.args.vike {
          config.push(("base", "\"./\"".into()));
        }
      }
      Target::Script => {
        imports.push(("{ resolve }", "node:path"));
        let build_config = match platform {
          Platform::Browser => {
            let name = self.args.name.as_global_var();
            format!(
              r#"lib: {{
  entry: resolve(__dirname, "src/{}"),
  name: "{name}",
  formats: ["iife"]
}},"#,
              entry_point.unwrap()
            )
          }
          Platform::Node => format!(
            r#"lib: {{
  entry: resolve(__dirname, "src/{}"),
  formats: ["cjs"]
}},
minify: false,"#,
            entry_point.unwrap()
          ),
        };

        let mut external = "Object.keys(manifest.dependencies || {})".to_string();
        if platform.is_node() {
          imports.push(("{ builtinModules }", "node:module"));
          external.push_str(".concat(builtinModules)");
        }

        let rollup_config = format!(
          r#"rollupOptions: {{
  external: {external}
}}"#
        );
        let full_obj = format!(
          "{{\n{}\n{}\n}}",
          textwrap::indent(&build_config, "  "),
          textwrap::indent(&rollup_config, "  ")
        );
        config.push(("build", full_obj.into()));
      }
      Target::Lib => {}
    }

    // This is needed for libraries like React that rely on process.env.NODE_ENV during bundling.
    config.push((
      "define",
      r#"{
  "process.env.NODE_ENV": JSON.stringify(mode)
}"#
        .into(),
    ));

    let mut plugins = Vec::new();
    if self.args.react {
      plugins.push("react()");
    }
    if self.args.vike {
      plugins.push("vike({ prerender: true })");
    }
    if !plugins.is_empty() {
      config.push(("plugins", format!("[{}]", plugins.join(", ")).into()));
    }

    // TODO: Revisit deps.inline once this issue is closed:
    // https://github.com/vitest-dev/vitest/issues/2806
    let test_config = format!(
      r#"{{
  environment: "{environment}",{setup_files}
  deps: {{
    inline: [/^(?!.*vitest).*$/]
  }}
}}"#
    );
    config.push(("test", test_config.into()));

    if platform.is_node() {
      config.push(("resolve", "{ conditions: [\"node\"] }".into()));
    }

    imports.sort_by_cached_key(|(_, path)| PackageName::from_str(path).unwrap());
    let imports_str = imports
      .into_iter()
      .map(|(l, r)| format!("import {l} from \"{r}\";\n"))
      .collect::<String>();
    let config_str = config
      .into_iter()
      .map(|(k, v)| textwrap::indent(&format!("{k}: {v}"), "  "))
      .collect::<Vec<String>>()
      .join(",\n");
    let mut src = format!(
      r#"{imports_str}
let manifest = JSON.parse(fs.readFileSync("package.json", "utf-8"));
export default defineConfig(({{ mode }}) => ({{
{config_str}
}}));
"#
    );

    if target.is_site() || target.is_script() {
      files.push(("vite.config.ts".into(), src.into()));
    } else {
      src.insert_str(0, "/// <reference types=\"vitest\" />\n");
      files.push(("vitest.config.ts".into(), src.into()));
    }

    files
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
    let path = ws.root.join("typedoc.json");
    let mut f = OpenOptions::new()
      .read(true)
      .write(true)
      .open(&path)
      .with_context(|| format!("Failed to open file: {}", path.display()))?;
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

  fn make_gitignore() -> FileVec {
    let gitignore = ["node_modules", "dist", "docs"].join("\n");
    vec![(".gitignore".into(), gitignore.into())]
  }

  fn run_pnpm(&self, f: impl Fn(&mut Command)) -> Result<()> {
    let mut cmd = Command::new(self.global_config.pnpm_path());
    f(&mut cmd);

    if self.args.offline {
      cmd.arg("--offline");
    }

    if self.args.prefer_offline {
      cmd.arg("--prefer-offline");
    }

    let status = cmd.status()?;
    ensure!(status.success(), "pnpm failed");
    Ok(())
  }

  fn install_ws_dependencies(&self, root: &Path, is_workspace: bool) -> Result<()> {
    #[rustfmt::skip]
    let ws_dependencies: Vec<&str> = vec![
      // Building
      "vite",

      // Testing
      "vitest",

      // Types
      "typescript",
      "@types/node",

      // Linting and formatting
      "@biomejs/biome",

      // Documentation generation
      "typedoc"
    ];

    self.run_pnpm(|pnpm| {
      pnpm.args(["add", "--save-dev"]).args(&ws_dependencies);
      if is_workspace {
        pnpm.arg("--workspace-root");
      }
      pnpm.current_dir(root);
    })?;

    Ok(())
  }

  fn make_index_html(js_entry_point: &str, css_entry_point: &str) -> String {
    format!(
      r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link href="/styles/{css_entry_point}" rel="stylesheet" type="text/css" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/{js_entry_point}"></script>
  </body>
</html>"#
    )
  }

  fn new_package(self, root: &Path) -> Result<()> {
    let NewArgs {
      name,
      target,
      platform,
      ..
    } = &self.args;

    let src_dir = root.join("src");
    utils::create_dir(src_dir)?;

    let tests_dir = root.join("tests");
    utils::create_dir(tests_dir)?;

    let mut manifest = pj::PackageJson::builder().build();
    manifest.name = Some(name.to_string());
    manifest.version = Some(String::from("0.1.0"));
    manifest.type_ = Some(pj::Type::Module);

    let mut other: IndexMap<String, Value> = IndexMap::new();
    let pkg_config = PackageDepotConfig {
      platform: *platform,
      target: Some(*target),
      ..Default::default()
    };
    let ws_config = WorkspaceDepotConfig {
      depot_version: DEPOT_VERSION.to_string(),
    };
    let mut config = serde_json::to_value(pkg_config)?;
    json_merge(&mut config, serde_json::to_value(ws_config)?);
    other.insert("depot".into(), config);

    // STUPID HACK:
    // - This npm bug (and I guess pnpm bug) causes platform-specific rollup packages to not be installed:
    //   https://github.com/npm/cli/issues/4828
    // - A stupid patch is to use the Wasm build of Rollup:
    //   https://github.com/vitejs/vite/issues/15167
    if self.ws_opt.is_none() {
      other.insert(
        "pnpm".into(),
        json!({
          "overrides": {
            "rollup": "npm:@rollup/wasm-node"
          }
        }),
      );
    }

    let mut files: FileVec = Vec::new();

    let mut peer_dependencies: Vec<&str> = Vec::new();
    let mut dev_dependencies: Vec<&str> = vec![];

    if platform.is_browser() {
      dev_dependencies.extend(["jsdom"]);
    }

    if self.args.react {
      dev_dependencies.extend([
        "react",
        "react-dom",
        "@types/react",
        "@types/react-dom",
        "@vitejs/plugin-react",
        "@testing-library/react",
      ]);
    }

    if self.args.vike {
      ensure!(
        target.is_site(),
        "--vike can only be used with --target site"
      );

      dev_dependencies.push("vike");

      if self.args.react {
        dev_dependencies.push("vike-react");
      }
    }

    if self.args.sass {
      dev_dependencies.push("sass");
    }

    let entry_point = match target {
      Target::Site => {
        ensure!(
          platform.is_browser(),
          "Must have platform=browser when target=site"
        );

        dev_dependencies.push("normalize.css");

        let css_name = if self.args.vike { "base" } else { "index" };
        let css_path = format!("{css_name}.{}", if self.args.sass { "scss" } else { "css" });

        if self.args.vike {
          ensure!(self.args.react, "Currently must use --react with --vike");
          const CONFIG_SRC: &str = r#"import vikeReact from "vike-react/config";
import type { Config } from "vike/types";

export let config: Config = {
  extends: vikeReact,
  lang: "en-US"
};
"#;
          files.push(("src/+config.ts".into(), CONFIG_SRC.into()));

          let head_src = format!(
            r#"import React from "react";
import "./{css_path}";

export let Head = () => (
  <>
    <meta name="viewport" content="width=device-width, initial-scale=1" />
  </>
);
"#
          );
          files.push(("src/+Head.tsx".into(), head_src.into()));
          files.push((format!("src/{css_path}").into(), CSS.into()));

          const INDEX_SRC: &str = r#"import React from "react";

export default () => {
  return <h1>Hello, world!</h1>;
};
"#;
          files.push(("src/index/+Page.tsx".into(), INDEX_SRC.into()));

          const TITLE_SRC: &str = r#"export let title = "Example Site";
"#;
          files.push(("src/index/+title.tsx".into(), TITLE_SRC.into()));
        } else {
          let (js_path, js_contents) = if self.args.react {
            ("index.tsx", REACT_INDEX)
          } else {
            ("index.ts", BASIC_INDEX)
          };

          files.push((
            "index.html".into(),
            Self::make_index_html(js_path, &css_path).into(),
          ));

          utils::create_dir(root.join("styles"))?;
          files.push((format!("styles/{css_path}").into(), CSS.into()));
          files.push((format!("src/{js_path}").into(), js_contents.into()));
        }

        None
      }
      Target::Script => {
        if platform.is_node() {
          manifest.bin = Some(pj::Binary::Object(indexmap! {
            name.name.clone() => format!("dist/{}.cjs", self.args.name)
          }));
          dev_dependencies.push("vite");
        }
        let filename = if self.args.react {
          "main.tsx"
        } else {
          "main.ts"
        };
        files.push((format!("src/{filename}").into(), MAIN.into()));

        Some(filename)
      }
      Target::Lib => {
        manifest.main = Some(String::from("dist/lib.js"));
        manifest.files = Some(vec![String::from("dist")]);

        if self.args.react {
          peer_dependencies.push("react");
        }

        let main_export = pj::ExportsObject::builder()
          .default("./dist/lib.js")
          .build();
        let sub_exports = pj::ExportsObject::builder().default("./dist/*.js").build();
        manifest.exports = Some(pj::Exports::Nested(indexmap! {
          ".".into() => main_export,
          "./*".into() => sub_exports,
        }));

        files.push(("tests/add.test.ts".into(), TEST.into()));

        match &self.ws_opt {
          Some(ws) => self.update_typedoc_config(ws)?,
          None => files.extend(self.make_typedoc_config()?),
        }

        let filename = if self.args.react { "lib.tsx" } else { "lib.ts" };
        files.push((format!("src/{filename}").into(), LIB.into()));

        Some(filename)
      }
    };

    manifest.other = Some(other);

    files.push((
      "package.json".into(),
      serde_json::to_string_pretty(&manifest)?.into(),
    ));
    files.extend(self.make_tsconfig()?);
    files.extend(self.make_biome_config()?);
    files.extend(self.make_vite_config(entry_point));

    if self.ws_opt.is_none() {
      files.extend(Self::make_gitignore());
    }

    for (rel_path, contents) in files {
      let abs_path = root.join(rel_path);
      utils::create_dir_if_missing(abs_path.parent().unwrap())?;
      utils::write(abs_path, contents.as_bytes())?;
    }

    if !peer_dependencies.is_empty() {
      self.run_pnpm(|pnpm| {
        pnpm
          .args(["add", "--save-peer"])
          .args(&peer_dependencies)
          .current_dir(root);
      })?;
    }

    if !dev_dependencies.is_empty() {
      self.run_pnpm(|pnpm| {
        pnpm
          .args(["add", "--save-dev"])
          .args(&dev_dependencies)
          .current_dir(root);
      })?;
    }

    match &self.ws_opt {
      Some(ws) => self.install_ws_dependencies(&ws.root, true)?,
      None => self.install_ws_dependencies(root, false)?,
    }

    Ok(())
  }

  pub fn run(self) -> Result<()> {
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
    utils::create_dir(&root)?;

    if self.args.workspace {
      self.new_workspace(&root)
    } else {
      self.new_package(&root)
    }
  }
}
