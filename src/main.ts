import "@cspotcode/source-map-support/register.js";
import * as cp from "child_process";
import { program } from "commander";
import path from "path";

import { registerCommands } from "./commands/mod";
import { gracoPkgRoot } from "./common";
import { log } from "./log";

declare global {
  var DEV_MODE: boolean;
  var VERSION: string;
}

program.name("graco").version(VERSION);
registerCommands(program);

let exec = (cmd: string) => {
  cp.execSync(cmd, { stdio: "inherit" });
};

// TODO: workspace support for these
let pnpmSynonyms = [
  {
    command: "add",
    description: "Add a new dependency",
  },
  {
    command: "update",
    description: "Update a dependency",
  },
  {
    command: "link",
    description: "Symlink a dependency",
  },
];
pnpmSynonyms.forEach(({ command, description }) => {
  program
    .command(command)
    .description(description + " (via pnpm)")
    .allowUnknownOption(true)
    .action((_flags, cmd) => exec(`pnpm ${command} ${cmd.args.join(" ")}`));
});

let gracoSynonyms = [
  {
    command: "commit-check",
    description: "Clean, init, build, and test",
    subcommands: ["clean", "init", "build", "test"],
  },
  {
    command: "prepare",
    description: "Init and build for production",
    subcommands: ["init", "build --release"],
  },
];
gracoSynonyms.forEach(({ command, description, subcommands }) => {
  let shell = subcommands
    .map(cmd => `node ${path.join(gracoPkgRoot, "dist", "main.mjs")} ${cmd}`)
    .join(" && ");
  program
    .command(command)
    .description(description)
    .action(() => exec(shell));
});

program.parseAsync(process.argv).catch(err => {
  log.error(DEV_MODE ? err.stack : err.message);
});
