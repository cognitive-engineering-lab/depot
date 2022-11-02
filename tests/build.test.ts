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
    let src = {
      "src/index.tsx": `export let foo = "bar";\n`,
      "index.html": `<html><script type="module" src="/src/index.tsx"></script></html>`,
    };
    return Graco.with({ src }, async graco => {
      expect(await graco.run("build")).toBe(0);
      graco.test("dist/index.html");
    });
  });

  it("supports monorepos", () => {
    let src = {
      "packages/foo/src/lib.ts": `export let foo = "bar";\n`,
      "packages/foo/package.json": `{"name": "foo", "version": "0.0.1", "main": "dist/lib.js"}`,
      "packages/bar/src/lib.ts": `import { foo } from "foo";\n\nconsole.log(foo);\n`,
      "packages/bar/package.json": `{"name": "bar", "dependencies": {"foo": "0.0.1"}}`,
    };
    return Graco.with({ src }, async graco => {
      expect(await graco.run("build")).toBe(0);
      graco.test("packages/foo/dist/lib.js");
      graco.test("packages/bar/dist/lib.js");
    });
  });
});
