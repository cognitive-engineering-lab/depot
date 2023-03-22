use graco_test_utils::project;

#[test]
fn fmt_basic() {
  let project = project().file("src/lib.ts", "let x = 1 +    2;").persist();
  project.graco("fmt");
  assert_eq!(project.read("src/lib.ts"), "let x = 1 + 2;\n");
}
