import "@cspotcode/source-map-support/register.js";
import { program } from "commander";

import { registerCommands } from "./commands/mod";
import { log } from "./log";

declare global {
  var DEV_MODE: boolean;
}

registerCommands(program);
program.parseAsync(process.argv).catch(err => {
  log.error(DEV_MODE ? err.stack : err.message);
});
