use graco_test_utils::{project, project_for};

#[test]
fn build_basic_lib_browser() {
  let project = project_for("lib", "browser").persist();
  project.graco("build");
  assert!(project.exists("dist/lib.js"));
  assert!(project.exists("dist/lib.d.ts"));
  assert!(project.exists("dist/lib.js.map"));
}

#[test]
fn build_basic_lib_node() {
  let project = project_for("lib", "node");
  project.graco("build");
  assert!(project.exists("dist/lib.js"));
  assert!(project.exists("dist/lib.js.map"));
}

#[test]
fn build_basic_script_browser() {
  let project = project_for("script", "browser");
  project.graco("build");
  assert!(project.exists("dist/main.js"));
  assert!(project.exists("dist/main.js.map"));
}

#[test]
fn build_basic_script_node() {
  let project = project_for("script", "node");
  project.graco("build");
  assert!(project.exists("dist/main.js"));
  assert!(project.exists("dist/main.js.map"));
}

#[test]
fn build_basic_site_browser() {
  let project = project_for("site", "browser");
  project.graco("build");
  assert!(project.exists("dist/index.html"));
}

#[test]
fn build_release() {
  let project = project();
  project.graco("build --release");
  assert!(project.exists("dist/lib.js"));
  assert!(project.exists("dist/lib.d.ts"));
  // Shouldn't generate source maps in release mode
  assert!(!project.exists("dist/lib.js.map"));
}
