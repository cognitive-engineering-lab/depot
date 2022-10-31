import { Graco } from "./harness";

test("clean works", () => {
  let src = `export let foo = "bar";\n`;
  return Graco.with({ src, debug: true }, async (graco) => {
    expect(await graco.run("build")).toBe(0);
    graco.test("dist/lib.js");

    expect(await graco.run("clean")).toBe(0);
    expect(() => graco.test("dist/lib.js")).toThrow();
  });
});
