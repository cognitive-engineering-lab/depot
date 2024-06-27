use depot_test_utils::project;

#[test]
fn formatting() {
  let p = project();
  p.depot("fmt --check");
}
