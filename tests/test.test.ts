import { Graco } from "./harness";

describe("test", () => {
  it("runs and checks tests", () => {
    let src = {
      "package.json": `{"name": "foo", "main": "dist/lib.js", "type": "module"}`,
      "src/lib.ts": "export let foo = 1;\n",
      "tests/lib.test.ts": `import {foo} from "../dist/lib.js";

test("ok", () => expect(foo).toBe(1));`,
    };
    return Graco.with({ src }, async graco => {
      expect(await graco.run("build")).toBe(0);
      expect(await graco.run("test")).toBe(0);
    });
  });

  it("fails if a test does not pass", () => {
    let src = {
      "package.json": `{"name": "foo", "main": "dist/lib.js", "type": "module"}`,
      "src/lib.ts": "export let foo = 1;\n",
      "tests/lib.test.ts": `import {foo} from "../dist/lib.js";

test("ok", () => expect(foo).toBe(0));`,
    };
    return Graco.with({ src }, async graco => {
      expect(await graco.run("build")).toBe(0);
      expect(await graco.run("test")).toBe(1);
    });
  });
});
