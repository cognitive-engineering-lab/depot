import "@cspotcode/source-map-support/register.js";
import { program } from "commander";

import * as build from "./build";

build.register();

program.parseAsync(process.argv);
