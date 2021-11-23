import * as log from "https://deno.land/std@0.114.0/log/mod.ts";
import docopt from "https://cdn.deno.land/docopt/versions/v1.0.7/raw/dist/docopt.mjs";
import * as _ from "https://deno.land/x/lodash@4.17.19/dist/lodash.js";
import { minimal } from "./minimal.ts";
import { toJson } from "./syntax/yaml.ts";

interface Opts {
  eval: boolean;
  run: boolean;
  fmt: boolean;
  "<expr>": string[];
  "<file>": string[];
}

const prog = "minimal";

const doc = `
MiniMAL interpreter

Usage:
  ${prog} <file>...
  ${prog} -h | --help                  Show usage
  ${prog} -v | --version               Show version

Options:
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

export async function main(argv = Deno.args) {
  const opts = getopts(argv);
  const m = minimal({});

  for (const file of opts["<file>"]) {
    const code: any = toJson(await Deno.readTextFile(file));
    for (const expr of code) {
      await m.eval(expr).catch(console.error);
    }
  }
}

if (import.meta.main) await main();
