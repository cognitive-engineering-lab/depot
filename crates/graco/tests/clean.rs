use graco_test_utils::{project, workspace_single_lib};

#[test]
fn clean_basic() {
  let p = project();
  p.graco("build");
  assert!(p.exists("dist"));
  p.graco("clean");
  assert!(!p.exists("dist"));
}

#[test]
fn clean_workspace() {
  let ws = workspace_single_lib();
  ws.graco("build");
  assert!(ws.exists("packages/bar/dist"));
  ws.graco("clean");
  assert!(!ws.exists("packages/bar/dist"));
}
