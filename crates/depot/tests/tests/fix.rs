use depot_test_utils::project;

#[test]
fn basic() {
  let p = project();
  p.file("src/lib.ts", r#"import {x} from "./foo";"#);
  p.file("src/foo.ts", "export let x = 0;");
  p.depot("fix");
  assert_eq!(p.read("src/lib.ts"), "");
}
