use graco_test_utils::project;

#[test]
fn test_basic() {
  let project = project();
  project.graco("test");
}

#[test]
fn test_should_fail() {
  // TODO!

  // let project = project();
  // project.graco("test");
}
