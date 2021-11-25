// (new Error).lineNumber

// import {log} from '/log.ts'
import * as YAML from "yaml2";
// import * as log from "https://deno.land/x/branch@0.1.6/mod.ts";
import { Papyrus } from "https://deno.land/x/papyrus/mod.ts";
// import { PapyrusPretty } from "https://deno.land/x/papyrus-pretty/mod.ts";

// colors for a pretty cli
import {
  blue,
  bold,
  gray,
  dim,
  green,
  italic,
  red,
  reset,
  setColorEnabled,
  underline,
  // inverse,
  yellow,
  white,
  brightBlack,
} from "https://deno.land/std@0.106.0/fmt/colors.ts";

// const logger = log.create("main");

const stringify = (msg: any) =>
  YAML.stringify(msg, {
    singleQuote: false,
    minContentWidth: 0,
    simpleKeys: true,
    flow: true,
    indentSeq: true,
    blockQuote: false,
    lineWidth: 0,
  });

const inspect = (obj: any) =>
  Deno.inspect(obj, {
    compact: true,
    colors: true,
    depth: 2,
  });

// .replaceAll(':', reset(gray(':')

// (substring: string, ...args: any[])

const date = (d: Date) => {
  const short = Deno.inspect(d).substr(11, 12);

  const colored = (dim((short)).replaceAll(
    /(?:^0|\:|\.[0-9]{3})/g,
    (s: string, ...a: any[]) => (brightBlack((s)))
  ));

  return colored;
};

import * as log from "https://deno.land/std/log/mod.ts";
await log.setup({
  //define handlers
  handlers: {
    console: new log.handlers.ConsoleHandler("DEBUG", {
      formatter: (rec: any) => {
        const parsed = YAML.parse(rec.msg);
        const msg =
          typeof parsed === "string"
            ? bold(underline(parsed))
            : inspect(parsed)
                .split(/\n/)
                .map((line, i) =>
                  i === 0 ? line : `${blue(("  |"))} ${line}`
                )
                .join("\n");

        return `${(rec.levelName.substr(0, 1).toLowerCase())}${blue(
          (" |")
        )} ${date(rec.datetime)} ${(((msg)))}`;
      },
    }),
    file: new log.handlers.RotatingFileHandler("INFO", {
      filename: "./var/a.log",
      maxBytes: 1500000,
      maxBackupCount: 5,
      formatter: (rec: any) =>
        JSON.stringify({
          // region: rec.loggerName,
          t: rec.datetime,
          level: rec.levelName,
          data: rec.msg,
        }),
    }),
  },

  //assign handlers to loggers
  loggers: {
    default: {
      level: "DEBUG",
      handlers: ["console"],
    },
    client: {
      level: "INFO",
      handlers: ["file"],
    },
  },
});
const dl = log.getLogger();
const cl = log.getLogger("client");
const arr = new Array(20).fill(1);
dl.debug("string message 1");
dl.info(10000000);
dl.warning(new Date());
dl.critical(arr);
dl.error({ a: 1, b: 2, msg: "foo", c: { d: "e", f: [1, 2, 3] } });
cl.debug("this should not come");
cl.info("string message 1");
cl.info(10000000);
cl.warning(new Date());
cl.critical(arr);
cl.error({ a: 1, b: 2, c: { d: "e", f: [1, 2, 3] } });

// export async function main() {

//   const logger = new Papyrus("myLogger");

//   const bindings = {
//     arch: Deno.build.arch,
//     os: Deno.build.os,
//     pid: Deno.pid,
//   }

//   // const logger = new Papyrus({
//   //   bindings,
//   //   mergeBindings: false,
//   //   formatter: new PapyrusPretty
//   // });

//   logger.info("This is an info");

//   logger.info("Hello World!");
//   // logger.info(123, {obj: 69})
//   const payload = {
//     x: 0.12,
//     y: 0.71,
//     z: 0.35,
//   }

//   logger.info("This is an info", payload);
// }

// await main();
