import { Graco } from "./harness";

describe("init", () => {
  it("installs packages", () =>
    Graco.with(
      { manifest: { dependencies: { lodash: "^4.17.15" } } },
      async graco => {
        await graco.run("init");
        graco.test("node_modules/lodash/package.json");
      }
    ));
});
