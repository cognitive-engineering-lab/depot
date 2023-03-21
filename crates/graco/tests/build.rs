use graco_test_utils::project_for;

#[test]
fn basic_lib_browser() {
  let project = project_for("lib", "browser");
  project.graco("build");
  assert!(project.exists("dist/lib.js"));
}

#[test]
fn basic_script_browser() {
  let project = project_for("script", "browser");
  project.graco("build");
  assert!(project.exists("dist/main.js"));
}

#[test]
fn basic_site_browser() {
  let project = project_for("site", "browser");
  project.graco("build");
  assert!(project.exists("dist/index.html"));
}
