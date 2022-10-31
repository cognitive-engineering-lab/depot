import { Graco } from "./harness";

describe("build", () => {
  it("compiles a simple file", () => {
    let src = `export let foo = "bar";\n`;
    return Graco.with({ src }, async (graco) => {
      expect(await graco.run("build")).toBe(0);
      graco.test("dist/lib.js");
    });
  });

  it("finds type errors with tsc", () => {
    let src = `export let foo: number = "bar";`;
    return Graco.with({ src }, async (graco) => {
      expect(await graco.run("build")).not.toBe(0);
    });
  });

  it("finds lint errors with ´slint", () => {
    let src = `export let foo    = "bar";`;
    return Graco.with({ src }, async (graco) => {
      expect(await graco.run("build")).not.toBe(0);
    });
  });

  it("runs vite to build a website", () => {
    let src = { "index.tsx": `export let foo = "bar";\n` };
    return Graco.with({ src }, async (graco) => {
      expect(await graco.run("build")).toBe(0);
      graco.test("dist/index.html");
    });
  });
});
