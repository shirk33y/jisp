#!/usr/bin/env -S denon run --allow-env --allow-write --allow-read --allow-net --import-map import_map.json --unstable
// #!/usr/bin/env -S deno run --allow-env --allow-write --allow-read --allow-net --import-map import_map.json --unstable
import docopt from "https://cdn.deno.land/docopt/versions/v1.0.7/raw/dist/docopt.mjs";
import * as log from "./log.ts";
import * as _ from "lodash";
import { jisp } from "./jisp.ts";
import * as minimal from "./syntax/minimal.ts";
import * as astfun from "./syntax/astfun.ts";
import { die, stdoutWrite } from "./utils.ts";

const parsers = { minimal, astfun };

type Syntax = keyof typeof parsers;

interface Opts {
  FILE: string[];
  "-s": Syntax;
  "-t": Syntax;
  "-l": string;
}

const prog = "jisp";

const doc = `
JISP interpreter & converter

Usage:
  ${prog} [-l <log-level>] [-s <syntax>] FILE ... 
  ${prog} [-l <log-level>] -t <target-syntax> [-s <syntax>] FILE ... 
  
  ${prog} -h | --help                  Show usage
  ${prog} --version                    Show version

Options:
  -s <syntax>         [default: minimal]
  -t <target-syntax>  Translate to syntax.
  -l <log-level>      [default: INFO]
  -h --help           Show this screen.
  --version           Show version.
`;

function getopts(argv: string[]) {
  try {
    return docopt(doc, { argv }) as Opts;
  } catch (error) {
    die(1, error.message);
  }
}

function parse(text: string, syntax: Syntax) {
  const parser = parsers[syntax]?.toMinimal;

  if (!parser) {
    die(32, `Unknown parser`, { syntax, parsers });
  }

  const ast = parser(text);

  if (!ast) {
    die(15, `Failed to parse ast`, { syntax, text });
  }

  return ast;
}

function translate(
  file: string,
  ast: any,
  syntax: Syntax,
  targetSyntax: Syntax
) {
  if (syntax === targetSyntax) {
    die(69, `Target syntax equals source syntax`, file, {
      syntax,
      targetSyntax,
    });
  }

  log.info(`Translating`, file, { syntax, targetSyntax });

  const stringifier = parsers[targetSyntax]?.fromMinimal;

  if (!stringifier) {
    die(33, `Unsupported syntax`, { targetSyntax, parsers });
  }

  return stringifier(JSON.stringify(ast));
}

async function evaluate(
  file: string,
  ast: any,
  syntax: Syntax,
  m: ReturnType<typeof jisp>
) {
  for (const expr of ast) {
    try {
      await m.eval(expr);
    } catch (error) {
      die(13, `Evaluate error`, error, { file, ast, syntax });
    }
  }
}

export async function main(argv = Deno.args) {
  const opts = getopts(argv);
  log.debug("Parsed options", opts);
  console.log("Parsed options", opts);
  const m = jisp({});
  let ast;

  await log.defaultSetup(opts["-l"] as log.LevelName);

  log.warning("Log level", opts["-l"]);

  for (const file of opts["FILE"]) {
    log.info("Reading file", file);

    try {
      const text = await Deno.readTextFile(file);
      ast = await parse(text, opts["-s"]);
    } catch (err: any) {
      die(4, `Error reading file`, file, err);
    }

    if (opts["-t"]) {
      const out = await translate(file, ast, opts["-s"], opts["-t"]);
      log.info("Translate success", { out });
      await stdoutWrite(out);
    } else {
      await evaluate(file, ast, opts["-s"], m);
    }
  }
}

if (import.meta.main) await main();
