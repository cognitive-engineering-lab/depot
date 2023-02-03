import { Graco } from "./harness";

describe("build", () => {
  it("compiles a simple file", () => {
    let src = `export let foo = "bar";\n`;
    return Graco.with({ src }, async graco => {
      expect(await graco.run("build")).toBe(0);
      graco.test("dist/lib.js");
    });
  });

  it("finds type errors with tsc", () => {
    let src = `export let foo: number = "bar";`;
    return Graco.with({ src }, async graco => {
      expect(await graco.run("build")).not.toBe(0);
    });
  });

  it("finds lint errors with eslint", () => {
    let src = `export let foo    = "bar";`;
    return Graco.with({ src }, async graco => {
      // TODO: expect to see lint output
      expect(await graco.run("build")).toBe(0);
    });
  });

  it("runs vite to build a website", () => {
    return Graco.with({ flags: "-t site" }, async graco => {
      expect(await graco.run("build")).toBe(0);
      graco.test("dist/index.html");
    });
  });

  it("supports monorepos", () => {
    return Graco.with({ flags: "-w" }, async graco => {
      expect(await graco.run("new foo")).toBe(0);
      expect(await graco.run("new bar")).toBe(0);
      expect(await graco.run("build")).toBe(0);
      graco.test("packages/foo/dist/lib.js");
      graco.test("packages/bar/dist/lib.js");
    });
  });
});
