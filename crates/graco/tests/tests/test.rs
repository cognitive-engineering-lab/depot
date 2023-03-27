use graco_test_utils::{project, workspace_single_lib};

#[test]
fn basic() {
  let p = project();
  p.graco("test");
}

#[test]
#[should_panic]
fn should_fail() {
  let p = project();
  p.file(
    "tests/fail.test.ts",
    r#"
import { add } from "bar";

test("add", () => expect(add(1, 2)).toBe(100))
  "#,
  );
  p.graco("test");
}

#[test]
fn workspace() {
  let ws = workspace_single_lib();
  ws.graco("test");
}

#[test]
#[should_panic]
fn workspace_should_fail() {
  let ws = workspace_single_lib();
  ws.file(
    "packages/bar/tests/fail.test.ts",
    r#"
import { add } from "bar";

test("add", () => expect(add(1, 2)).toBe(100))
  "#,
  );
  ws.graco("test");
}
