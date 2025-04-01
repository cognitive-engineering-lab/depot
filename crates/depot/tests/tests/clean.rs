use depot_test_utils::{project, workspace_single_lib};

#[test]
fn basic() {
    let p = project();
    p.depot("build");
    assert!(p.exists("dist"));
    p.depot("clean");
    assert!(!p.exists("dist"));
}

#[test]
fn workspace() {
    let ws = workspace_single_lib();
    ws.depot("build");
    assert!(ws.exists("packages/bar/dist"));
    ws.depot("clean");
    assert!(!ws.exists("packages/bar/dist"));
}
