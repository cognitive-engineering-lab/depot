import "@cspotcode/source-map-support/register.js";
import * as cp from "child_process";
import { program } from "commander";

import { registerCommands } from "./commands/mod";
import { log } from "./log";

declare global {
  var DEV_MODE: boolean;
}

registerCommands(program);
program.command("commit-check").action(() => {
  cp.execSync("graco clean && graco init && graco build && graco test", {
    stdio: "inherit",
  });
});
program.command("prepare").action(() => {
  cp.execSync("graco init && graco build", { stdio: "inherit" });
});
program.parseAsync(process.argv).catch(err => {
  log.error(DEV_MODE ? err.stack : err.message);
});
