use graco_test_utils::project;

#[test]
fn formatting() {
  let p = project();
  p.graco("fmt -- --check");
}
