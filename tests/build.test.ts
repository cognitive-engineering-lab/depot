import { Graco } from "./harness";

describe("build", () => {
  it("compiles a simple file", () => {
    let src = `export let foo = "bar";`;
    Graco.with({ src }, async (graco) => {
      await graco.run("build");
    });
  });

  test("finds type errors with tsc", () => {
    let src = `export let foo: number = "bar";`;
    Graco.with({ src }, async (graco) => {
      await expect(graco.run("build")).rejects.toThrow();
    });
  });
});
