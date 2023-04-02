use graco_test_utils::{project, project_for, workspace};

#[test]
fn basic_lib_browser() {
  let p = project_for("lib", "browser");
  p.graco("build");
  assert!(p.exists("dist/lib.js"));
  assert!(p.exists("dist/lib.d.ts"));
  assert!(p.exists("dist/lib.js.map"));
}

#[test]
fn basic_lib_node() {
  let p = project_for("lib", "node");
  p.graco("build");
  assert!(p.exists("dist/lib.js"));
  assert!(p.exists("dist/lib.js.map"));
}

#[test]
fn basic_script_browser() {
  let p = project_for("script", "browser");
  p.graco("build");
  assert!(p.exists("dist/main.mjs"));
  assert!(p.exists("dist/main.mjs.map"));
}

#[test]
fn basic_script_node() {
  let p = project_for("script", "node");
  p.graco("build");
  assert!(p.exists("dist/main.js"));
  assert!(p.exists("dist/main.js.map"));
}

#[test]
fn basic_site_browser() {
  let project = project_for("site", "browser");
  project.graco("build");
  assert!(project.exists("dist/index.html"));
}

#[test]
fn release() {
  let p = project();
  p.graco("build --release");
  assert!(p.exists("dist/lib.js"));
  assert!(p.exists("dist/lib.d.ts"));
  // Shouldn't generate source maps in release mode
  assert!(!p.exists("dist/lib.js.map"));
}

#[test]
fn workspace_() {
  let ws = workspace();
  ws.graco("new foo");
  ws.graco("new bar");

  // TODO: nicer way of editing package.json
  ws.file(
    "packages/bar/package.json",
    r#"{
  "dependencies": {"foo": "workspace:^0.1.0"},
  "graco": {"platform": "browser"}
}"#,
  );
  ws.graco("build");
  assert!(ws.exists("packages/foo/dist/lib.js"));
  assert!(ws.exists("packages/bar/dist/lib.js"));
}
