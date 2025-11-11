use std::process::Command;

use depot_test_utils::{custom_project_for, project, project_for, workspace};

#[test]
fn basic_lib_browser() {
  let p = project_for("lib", "browser");
  p.file("src/nested/foobar.css", ".red { color: red; }");
  p.depot("build --lint-fail");
  assert!(p.exists("dist/lib.js"));
  assert!(p.exists("dist/lib.d.ts"));
  assert!(p.exists("dist/lib.js.map"));
  assert!(p.exists("dist/nested/foobar.css"));
}

#[test]
fn basic_lib_browser_react() {
  let p = custom_project_for("lib", "browser", "--react").persist();
  p.depot("build --lint-fail");
  assert!(p.exists("dist/lib.js"));
  assert!(p.exists("dist/lib.d.ts"));
  assert!(p.exists("dist/lib.js.map"));
}

#[test]
fn basic_lib_node() {
  let p = project_for("lib", "node");
  p.depot("build --lint-fail");
  assert!(p.exists("dist/lib.js"));
  assert!(p.exists("dist/lib.js.map"));
}

#[test]
fn basic_script_browser() {
  let p = project_for("script", "browser");
  p.depot("build --lint-fail");
  assert!(p.exists("dist/foo.iife.js"));
  assert!(p.exists("dist/foo.iife.js.map"));
}

#[test]
fn basic_script_node() {
  let p = project_for("script", "node");
  p.depot("build --lint-fail");
  assert!(p.exists("dist/foo.cjs"));
  assert!(p.exists("dist/foo.cjs.map"));
}

#[test]
fn basic_site_browser() {
  let project = project_for("site", "browser");
  project.depot("build --lint-fail");
  assert!(project.exists("dist/index.html"));
}

#[test]
fn basic_site_sass() {
  let project = custom_project_for("site", "browser", "--sass");
  project.depot("build --lint-fail");
  assert!(project.exists("dist/index.html"));
}

#[test]
fn basic_site_vike() {
  let project = custom_project_for("site", "browser", "--vike --react");
  project.depot("build");
  assert!(project.exists("dist/client/index.html"));
}

#[test]
fn copy_assets() {
  let p = project();
  p.file("src/assets/foo.txt", "");
  p.file("src/styles/bar.css", "");
  p.depot("build");
  assert!(p.exists("dist/assets/foo.txt"));
  assert!(p.exists("dist/styles/bar.css"));
}

#[test]
fn release() {
  let p = project();
  p.depot("build --release");
  assert!(p.exists("dist/lib.js"));
  assert!(p.exists("dist/lib.d.ts"));
  // Shouldn't generate source maps in release mode
  assert!(!p.exists("dist/lib.js.map"));
}

#[test]
fn workspace_() {
  let ws = workspace();
  ws.depot("new foo");
  ws.depot("new bar");

  // TODO: nicer way of editing package.json
  ws.file(
    "packages/bar/package.json",
    r#"{
  "dependencies": {"foo": "workspace:^0.1.0"},
  "depot": {"platform": "browser"}
}"#,
  );

  ws.depot("init -- --no-frozen-lockfile");

  ws.depot("build");
  assert!(ws.exists("packages/foo/dist/lib.js"));
  assert!(ws.exists("packages/bar/dist/lib.js"));
}

#[test]
fn lint_basic() {
  let p = project();
  p.file("src/foo.ts", "export let x      = 1;");
  assert!(p.maybe_depot("build --lint-fail").is_err());
}

#[test]
fn lint_gitignore_basic() {
  let p = project();
  p.file("src/foo.ts", "export let x      = 1;");
  p.file(".gitignore", "foo.ts");
  let mut git = Command::new("git");
  assert!(
    git
      .current_dir(p.root())
      .arg("init")
      .status()
      .unwrap()
      .success()
  );
  p.depot("build");
  assert!(p.maybe_depot("build --lint-fail").is_ok());
}

#[test]
fn lint_gitignore_nested() {
  let p = project();
  p.file("src/foo.ts", "export let x      = 1;");
  p.file("src/.gitignore", "foo.ts");
  let mut git = Command::new("git");
  assert!(
    git
      .current_dir(p.root())
      .arg("init")
      .status()
      .unwrap()
      .success()
  );
  p.depot("build");
  assert!(p.maybe_depot("build --lint-fail").is_ok());
}

#[test]
fn vite_imports() {
  let p = project();
  p.file("src/foo.txt", "Hello world");
  p.file("src/lib.ts", r#"import _contents from "./foo.txt?raw";"#);
  p.depot("build");
}

#[test]
fn react_import() {
  let p = custom_project_for("lib", "browser", "--react");
  p.file("src/lib.tsx", r#"import ReactDOM from "react-dom/client";"#);
  p.depot("build");
}