import docopt from "https://cdn.deno.land/docopt/versions/v1.0.7/raw/dist/docopt.mjs";
import * as _ from "https://deno.land/x/lodash@4.17.19/dist/lodash.js";
import { jisp } from "./jisp.ts";
import * as minimal from "./syntax/minimal.ts";
import * as astfun from "./syntax/astfun.ts";

const parsers = { minimal, astfun };

type Syntax = keyof typeof parsers;

interface Opts {
  run: boolean;
  load: boolean;
  dump: boolean;
  FILE: string[];
  "-s": Syntax;
}

const prog = "jisp";

const doc = `
JISP interpreter & converter

Usage:
  ${prog} run [-s <syntax>] FILE ... 
  ${prog} load [FILE] [-s <syntax>]
  ${prog} dump [FILE] [-s <syntax>]
  ${prog} -h | --help                  Show usage
  ${prog} -v | --version               Show version

Options:
  -s <syntax>       [default: minimal]
  -h --help         Show this screen.
  --version         Show version.
`;

function getopts(argv: string[]) {
  try {
    return docopt(doc, { argv }) as Opts;
  } catch (error) {
    console.log(error.message);
    Deno.exit(1);
  }
}

function parse(text: string, syntax: Syntax) {
  const ast = parsers[syntax]?.toMinimal(text);

  if (!ast) {
    console.error(`Failed to parse: ${text}`);
    Deno.exit(1);
  }

  return ast;
}

export async function main(argv = Deno.args) {
  const opts = getopts(argv);
  const m = jisp({});

  for (const file of opts["FILE"]) {
    let code;
    try {
      code= await parse(await Deno.readTextFile(file), opts["-s"]);
    } catch (err: any) {
      console.log(Object.keys(err), err);
      continue
    }

    for (const expr of code) {
      // await m.eval(expr).catch(console.debug);
      await m.eval(expr).catch((err: any) => {
        console.log(Object.keys(err), err);
      });
    }
  }
}

if (import.meta.main) await main();
