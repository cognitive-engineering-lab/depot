use depot_test_utils::{project, workspace_single_lib};

#[test]
fn basic() {
    let p = project();
    p.depot("doc");
    assert!(p.exists("docs/index.html"));
    assert!(p.exists("docs/functions/add.html"));
}

#[test]
fn workspace() {
    let ws = workspace_single_lib();
    ws.depot("doc");
    assert!(ws.exists("docs/index.html"));
    assert!(ws.exists("docs/functions/add.html"));
}
