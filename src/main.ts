import "@cspotcode/source-map-support/register.js";
import { program } from "commander";

import { BuildCommand } from "./build";
import { FmtCommand } from "./fmt";

BuildCommand.register(program);
FmtCommand.register(program);

program.parseAsync(process.argv);
