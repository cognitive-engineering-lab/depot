import { Graco } from "./harness";

describe("clean", () => {
  it("removes dist files", () => {
    let src = `export let foo = "bar";\n`;
    return Graco.with({ src }, async graco => {
      expect(await graco.run("build")).toBe(0);
      graco.test("dist/lib.js");

      expect(await graco.run("clean")).toBe(0);
      expect(() => graco.test("dist/lib.js")).toThrow();
    });
  });

  it("removes config files with -a", () => {
    let src = `export let foo = "bar";\n`;
    return Graco.with({ src }, async graco => {
      graco.test(".eslintrc.cjs");
      expect(await graco.run("clean -a")).toBe(0);
      expect(() => graco.test(".eslintrc.cjs")).toThrow();
    });
  });
});
