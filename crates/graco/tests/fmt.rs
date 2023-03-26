use graco_test_utils::{project, workspace_single_lib};

#[test]
fn fmt_basic() {
  let p = project();
  p.file("src/lib.ts", "let x = 1 +    2;");
  p.graco("fmt");
  assert_eq!(p.read("src/lib.ts"), "let x = 1 + 2;\n");
}

#[test]
fn fmt_workspace() {
  let ws = workspace_single_lib().persist();
  ws.file("packages/bar/src/lib.ts", "let x = 1 +    2;");
  ws.graco("fmt");
  assert_eq!(ws.read("packages/bar/src/lib.ts"), "let x = 1 + 2;\n");
}
