import { Graco } from "./harness";

describe("test", () => {
  it("runs and checks tests", () => {
    return Graco.with({ }, async graco => {
      expect(await graco.run("build")).toBe(0);
      expect(await graco.run("test")).toBe(0);
    });
  });

  it("fails if a test does not pass", () => {
    let src = {
      "tests/add.test.ts": `
import {add} from "example";

test("add", () => expect(add(1, 1)).toBe(3));`,
    };
    return Graco.with({ src }, async graco => {
      expect(await graco.run("build")).toBe(0);
      expect(await graco.run("test")).toBe(1);
    });
  });
});
