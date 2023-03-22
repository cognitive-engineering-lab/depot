use graco_test_utils::project;

#[test]
fn clean_basic() {
  let project = project();
  project.graco("build");
  assert!(project.exists("dist"));
  project.graco("clean");
  assert!(!project.exists("dist"));
}
