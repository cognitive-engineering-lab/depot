import { Graco } from "./harness";

test("fmt works", () => {
  let src = `
export let foo = 
  "bar";  
`;
  return Graco.with({ src }, async graco => {
    expect(await graco.run("fmt")).toBe(0);
    expect(graco.read("src/lib.ts")).toBe(`export let foo = "bar";\n`);
  });
});
