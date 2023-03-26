use graco_test_utils::{project, workspace_single_lib};

#[test]
fn doc_basic() {
  let p = project();
  p.graco("doc");
  assert!(p.exists("docs/index.html"));
}

#[test]
fn doc_workspace() {
  let ws = workspace_single_lib();
  ws.graco("doc");
  assert!(ws.exists("docs/index.html"));
}
