import "@cspotcode/source-map-support/register.js";
import { program } from "commander";

import { BuildCommand } from "./build";
import { FmtCommand } from "./fmt";
import { CleanCommand } from "./clean";
import { InitCommand } from "./init";

BuildCommand.register(program);
FmtCommand.register(program);
CleanCommand.register(program);
InitCommand.register(program);

program.parseAsync(process.argv);
