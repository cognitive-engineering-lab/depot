use depot_test_utils::{project, workspace_single_lib};

#[test]
fn basic() {
  let p = project();
  p.file("src/lib.ts", "let x = 1 +    2;");
  p.depot("fmt");
  assert_eq!(p.read("src/lib.ts"), "let x = 1 + 2;\n");
}

#[test]
fn workspace() {
  let ws = workspace_single_lib();
  ws.file("packages/bar/src/lib.ts", "let x = 1 +    2;");
  ws.depot("fmt");
  assert_eq!(ws.read("packages/bar/src/lib.ts"), "let x = 1 + 2;\n");
}
