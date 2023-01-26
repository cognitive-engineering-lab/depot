import { Graco } from "./harness";

test("prepare inits and builds a workspace", () => {
  let src = `export let foo = "bar";\n`;
  return Graco.with({ src }, async graco => {
    // TODO: test prepare w/ modules
    expect(await graco.run("prepare")).toBe(0);
    graco.test("dist/lib.js");
  });
});

test("commit-check cleans, inits, builds, and tests a workspace", () => {
  let src = `export let foo = "bar";\n`;
  return Graco.with({ src }, async graco => {
    // TODO: test other aspects of commit-check besides building
    expect(await graco.run("commit-check")).toBe(0);
    graco.test("dist/lib.js");
  });
});

test("add creates new dependencies", () => {
  let src = {
    "src/lib.ts": `export let foo = "bar";\n`,
    "package.json": `{"name": "foo", "version": "0.0.1", "main": "dist/lib.js"}`,
  };
  return Graco.with({ src }, async graco => {
    expect(await graco.run("add -D lodash")).toBe(0);
    graco.test("node_modules/lodash");
  });
});
