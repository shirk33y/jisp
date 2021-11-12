import * as log from "https://deno.land/std@0.114.0/log/mod.ts";
import {printf, sprintf} from "https://deno.land/std@0.114.0/fmt/printf.ts";
import docopt from "https://cdn.deno.land/docopt/versions/v1.0.7/raw/dist/docopt.mjs";
import * as _ from "https://deno.land/x/lodash@4.17.19/dist/lodash.js";
import {assert} from "https://deno.land/std@0.114.0/testing/asserts.ts";
import * as yaml from "https://deno.land/std@0.114.0/encoding/yaml.ts";

import { str} from '/std.ts'

console.log(str)

type Fn = {(): any; _MACRO?: number; _AST?: Ast};
type Scalar = string | boolean | number | null | Fn | Env;
type Ast = Array<Ast | Scalar>;
type AstNode = Scalar | Ast;
interface Env {
  [bind: string]: any;
}
type Exprs = Array<any>;

function fatal(...msg: any[]) {
  log.error("fatal: ", ...msg);

  return new Error(msg.map((m) => `${m}`).join(" "));
}

export default function minimal(E: Env) {
  // Evaluate the form/ast
  function _eval(ast: AstNode, env: Env) {
    return Array.isArray(ast) // list?
      ? ast.map((...a: AstNode[]) => _loop(a[0], env)) // list
      : typeof ast == "string" // symbol?
      ? ast in env // symbol in env?
        ? env[ast] // lookup symbol
        : E.throw(`undefined symbol: ${ast}`) // undefined symbol
      : ast; // ast unchanged
  }

  // Return new Env with symbols in ast bound to
  // corresponding values in exprs
  function _bind(ast: AstNode, env: Env, exprs: Exprs) {
    if (!Array.isArray(ast)) throw fatal(`args must be array`);
    env = Object.create(env);
    assert(Array.isArray(ast));
    ast?.some((a: any, i: number) =>
      a == "&"
        ? (env[ast[i + 1] as string] = exprs.slice(i))
        : ((env[a] = exprs[i]), 0)
    );
    return env;
  }

  function _set(env: Env, key: string, value: any): Env {
    return (env[key] = value);
  }

  function _macroExpand(ast: AstNode, env: Env) {
    assert(Array.isArray(ast));
    const [a0, ...a$] = ast;
    assert(typeof a0 === "string");

    while (a0 in env && env[a0]._MACRO) {
      ast = env[a0](a$);
    }
    return ast;
  }

  function _loop(ast: AstNode, env: Env): AstNode {
    while (true) {
      log.debug(`eval: ${ast}`);
      if (!Array.isArray(ast)) return _eval(ast, env);

      // apply
      ast = _macroExpand(ast, env);
      if (!Array.isArray(ast)) return _eval(ast, env);

      const [a0, a1, a2, a3] = [...ast];

      switch (a0) {
        // update current environment
        case "def":
          assert(typeof a1 === "string");
          return _set(env, a1, _loop(a2, env));

        // define new function (lambda)
        //   fn [arg1 arg2 & rest] body
        case "fn": {
          const f: Fn = (...a: any[]) => _loop(a2, _bind(a1, env, a));

          f._AST = [a2, env, a1];
          return f;
        }

        // new environment with bindings
        case "let": {
          assert(Array.isArray(a1));
          env = Object.create(env);

          for (let i = 0; i < a1.length; i++) {
            if (i % 2) {
              const k = a1[i - 1];
              assert(typeof k === "string");
              const v = a1[i];
              env[k] = _loop(v, env);
            }
          }
          ast = a2;
          break;
        }

        // branching conditional
        case "if":
          ast = _loop(a1, env) ? a2 : a3;
          break;

        // multiple forms (for side-effects)
        case "do":
          _eval(ast.slice(1, ast.length - 1), env);
          ast = ast[ast.length - 1];
          break;

        // quote (unevaluated)
        case "str":
        case "`":
          return a1;

        // mark as macro
        case "~": {
          const fn: Fn = _loop(a1, env) as Fn; // eval regular function
          fn._MACRO = 1; // mark as macro
          return fn;
        }

        // get or set attribute
        case ".-": {
          const [e0, e1, e2] = _eval(ast.slice(1), env);

          return e2 !== undefined
            ? (e0[e1] = e2) // set
            : e0[e1]; // get;
        }

        // call object method
        case ".": {
          const [e0, e1, ...e$] = _eval(ast.slice(1), env);

          return e0[e1].apply(e0, e$);
        }

        // try/catch
        case "try":
          try {
            return _loop(a1, env);
          } catch (err) {
            assert(Array.isArray(a2));
            return _loop(a2[2], _bind([a2[2]], env, [err]));
          }

        // invoke list form
        default: {
          const [e0, ...e$] = _eval(ast, env);

          if (e0._AST) {
            ast = e0._AST[0];
            env = _bind(e0._AST[2], e0._AST[1], e$);
          } else {
            return e0(...e$);
          }
        }
      }
    }
  }

  E = Object.assign(Object.create(E), {
    // Core
    js: eval,
    eval: (...a: AstNode[]) => _loop(a[0], E),
    assert,
    throw: (...a: AstNode[]) => {
      throw a[0];
    },

    // Console
    info: (msg: any, ...a: any[]) => log.info(msg, ...a),
    print: (...a: any[]) => console.log(...a),
    printf: (...a: any[]) => console.log(...a),
    sprintf,

    // Arithmetics
    "==": (a: number, b: number) => a === b,
    "<": (a: number, b: number) => a < b,
    ">": (a: number, b: number) => a > b,
    "+": (a: number, b: number) => a + b,
    "-": (a: number, b: number) => a - b,
    "*": (a: number, b: number) => a * b,
    "**": (a: number, b: number) => Math.pow(a, b),
    "/": (a: number, b: number) => a / b,
    "//": (a: number, b: number) => Math.floor(a / b),
    "%": (a: number, b: number) => a % b,

    // More math
    log: (a: number, b: number) => Math.log(a) / Math.log(b),
    floor: Math.floor,
    ceil: Math.ceil,
    round: Math.round,
    random: Math.random,
    sin: Math.sin,
    sinh: Math.sinh,
    cos: Math.cos,
    cosh: Math.cosh,
    atan: Math.atan,
    atan2: Math.atan2,

    "num/cos": "tbd?",

    // Type constructors
    str: (a: any) => a?.toString() || "",
    int: (a: any) => parseInt(a, 10),
    float: (a: any) => parseFloat(a),
    list: (a: any) => Array.from(a),
    obj: (...a: AstNode[]) => {
      const o: Record<string, any> = {};
      for (let i = 0; i < a?.length; i++) {
        if (i % 2) {
          const [k, v] = [a[i - 1], a[i]];
          assert(typeof k === "string");
          o[k] = v;
        }
      }
      return o;
    },

    // Type checkers
    "is/fn": (a: any) => typeof a === "function",
    "is/int": (n: any) => n === +n && n === (n | 0),
    "is/float": (n: any) => n === +n && n !== (n | 0),
    "is/str": (a: any) => typeof a === "string",
    "is/list": (a: any) => Array.isArray(a),
    "is/obj": (a: any) =>
      typeof a === "object" && !Array.isArray(a) && a !== null,

    // String ops
    "str/new": (s: any = "") => `${s}`,
    "str/is": (s: any) => typeof s === "string",
    "str/cat": (...s: string[]) => s.join(""),
    "str/fmt": (f: string, ...a: any[]) => sprintf(f, ...a),
    "str/len": (f: string) => f.length,
    "str/upper": (f: string) => f.toUpperCase(),
    "str/lower": (f: string) => f.toLowerCase(),
    "str/to-json": (s: string) => JSON.stringify(s),

    // List ops
    "list/map": (a: (...args: any) => any, b: Array<any>) => b.map((x) => a(x)),
    "list/filter": (a: (...args: any) => any, b: Array<any>) =>
      b.filter((x) => a(x)),
    "list/some": (a: (...args: any) => any, b: Array<any>) =>
      b.some((x) => a(x)),
    "list/every": (a: (...args: any) => any, b: Array<any>) =>
      b.every((x) => a(x)),
    "list/join": (delim: string, ...arr: string[]) => arr.join(delim),
    "list/has": (l: any[], item: any) => l.includes(item),
    "list/cat": (l: any[], ...c: any[][]) => l.concat(...c),
    "list/to-json": (l: any[]) => JSON.stringify(l),

    "#": (...a: AstNode[]) => "really this is obj?",

    // Object ops
    "obj/new": (...a: AstNode[]) => {
      const o: Record<string, any> = {};
      for (let i = 0; i < a?.length; i++) {
        if (i % 2) {
          const [k, v] = [a[i - 1], a[i]];
          assert(typeof k === "string");
          o[k] = v;
        }
      }
      return o;
    },
    "obj/is": (a: any) =>
      typeof a === "object" && !Array.isArray(a) && a !== null,
    "obj/items": (o: object) => Object.entries(o),
    "obj/get": (o: object, ...p: string[]) => _.get(o, p),
    "obj/keys": (o: object) => Object.keys(o),
    "obj/values": (o: object) => Object.values(o),
    "obj/has": (o: object, k: any) => k in o,
    "obj/cat": (o: object, ...c: object[]) => Object.assign(o, ...c),
    "obj/to-json": (o: object) => JSON.stringify(o),

    // Date ops
    "date/new": (ts: number) => new Date(ts),
    "date/from-str": (s: string, f: string) => {
      throw "tbd";
    },
    "date/is": (d: any) => d instanceof Date,
    "date/+": (d: Date, i: number, p: string) => {
      throw "tbd";
    },
    "date/-": (d: Date, i: number, p: string) => {
      throw "tbd";
    },
    "date/fmt": (d: Date, f: string) => {
      throw "tbd";
    },

    // File ops
    "file/new": () => null,
    "file/read": () => null,
    "file/read-line": () => null,
    "file/open": () => null,
    "file/exists": () => null,
    "file/close": () => null,
    "file/write": () => null,
    "file/delete": () => null,
    "file/touch": () => null,
    "file/move": () => null,

    // Path ops
    "path/new": () => null,
    "path/base": () => null,
    "path/dir": () => null,
    "path/rel": () => null,
    "path/abs": () => null,
    "path/join": () => null,

    // Url ops
    // in fact url = file/* + path/*
    "url/new": () => null,
    "url/from-path": () => null,
    "url/base": () => null,
    "url/dir": () => null,
    "url/join": () => null,
    "url/host": () => null,
    "url/port": () => null,
    "url/scheme": () => null,
    "url/path": () => null,
    "url/query": () => null,
    "url/param": () => null,
    "url/params": () => null,
    "url/fragment": () => null,
    "url/fetch": () => null,
    "url/head": () => null,
    "url/get": () => null,
    "url/post": () => null,
    "url/put": () => null,
    "url/options": () => null,
    "url/delete": () => null,

    // RPC
    "rpc/new": "tbd",
    "rpc/send": "tbd",
    "rpc/recv": "tbd",

    // UI / Web Component
    "ui/new": "tbd",
    "ui/render": "tbd",
    "ui/effect": "tbd",
    "ui/state": "tbd",
    "ui/dispatch": "tbd",

    "store/new": "tbd",
    "store/get": "tbd",
    "store/dispatch": "tbd",
    "store/reduce": "tbd",
    "store/saga": "tbd",
    "store/run": "tbd",
    // Candidates
    // isa: (...a: ScalarOrAst[]) => a[0] instanceof a[1],
    // type: (...a: ScalarOrAst[]) => typeof a[0],
    // new: (...a: AstNode[]) => new (a[0].bind(...a))(),
    // del: (...a: ScalarOrAst[]) => delete a[0][a[1]],
    // "list":  (...a: AstNode[]) => a,
    // read: (...a: ScalarOrAst[]) => JSON.parse(a[0]),
    // rep: (...a: ScalarOrAst[]) => JSON.stringify(EVAL(JSON.parse(a[0]), E)),
  });

  // Lib specific
  return E;
}

const prog = "minimal";

const doc = `
MiniMAL interpreter

Usage:
  ${prog} run <file>...
  ${prog} -h | --help                  Show usage
  ${prog} -v | --version               Show version

Options:
  -h --help         Show this screen.
  -o --out-format   Any of: yaml, json, lisp.
  --version         Show version.

`;

interface Opts {
  eval: boolean;
  run: boolean;
  fmt: boolean;
  "<expr>": string[];
  "<file>": string[];
}

if (import.meta.main) {
  let opts: Opts;

  try {
    opts = docopt(doc) as Opts;
  } catch (error) {
    if (Deno.args.length === 0) {
      log.error(`No command supplied`);
    } else {
      log.error("Invalid options");
    }
    console.log(error.message);
    Deno.exit(1);
  }

  const m = minimal({});

  if (opts.eval) {
    m.eval(opts["<expr>"]);
  } else if (opts.run) {
    for (const file of opts["<file>"]) {
      const code: any = yaml.parse(await Deno.readTextFile(file));
      for (const expr of code) {
        m.eval(expr);
      }
    }
  }
}
