import { Graco } from "./harness";

test("fmt works", () => {
  let src = `
export let foo = 
  "bar";  
`;
  Graco.with({ src }, async (graco) => {
    await graco.run("fmt");
    expect(graco.read("src/lib.ts")).toBe(`export let foo = "bar";\n`);
  });
});
