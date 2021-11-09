import * as log from "https://deno.land/std@0.113.0/log/mod.ts";
import {printf, sprintf} from "https://deno.land/std@0.113.0/fmt/printf.ts";
import docopt from "https://cdn.deno.land/docopt/versions/v1.0.7/raw/dist/docopt.mjs";
import * as _ from "https://deno.land/x/lodash@4.17.19/dist/lodash.js";
import {assert} from "https://deno.land/std@0.113.0/testing/asserts.ts";

type Scalar = string | boolean | number | null;
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

function at(ast: any, ...[index, ...next]: number[]): any {
  if (index === undefined) return ast;
  if (Array.isArray(ast)) return at(ast[index], ...next);

  throw fatal("expecting array, got:", ast);
}

// function cast()

function symbolAt(ast: any, ...path: number[]): string {
  const sym = at(ast, ...path);

  if (typeof sym === "string") return sym;
  if (typeof sym === "boolean") {
    log.warning(`boolean used as symbol name:`, ast);
    return `${sym}`;
  }

  throw fatal(`${typeof sym} used as symbol name:`, ast);
}

export default function minimal(E: Env) {
  // 2 args: eval_ast, 3 args: env_bind
  const evalOrBind = function (ast: AstNode, env: Env, exprs?: Exprs) {
    if (exprs) {
      // Return new Env with symbols in ast bound to
      // corresponding values in exprs
      if (!Array.isArray(ast)) throw fatal(`args must be array`);
      env = Object.create(env);
      ast?.some((a: any, i: number) =>
        a == "&"
          ? (env[symbolAt(ast, i + 1)] = exprs.slice(i))
          : ((env[a] = exprs[i]), 0)
      );
      return env;
    }
    // Evaluate the form/ast
    return Array.isArray(ast) // list?
      ? ast.map((...a: AstNode[]) => evalLoop(a[0], env)) // list
      : typeof ast == "string" // symbol?
      ? ast in env // symbol in env?
        ? env[ast] // lookup symbol
        : E.throw(ast + " not found") // undefined symbol
      : ///: null[ast]                          // undefined symbol
        ast; // ast unchanged
  };

  function setEnv(env: Env, key: string, value: any) {
    return (env[key] = value);
  }

  function macroExpand(ast: AstNode, env: Env) {
    while (
      Array.isArray(ast) &&
      symbolAt(ast, 0) in env &&
      env[symbolAt(ast, 0)].M
    ) {
      ast = env[symbolAt(ast, 0)](...ast.slice(1));
    }
    return ast;
  }

  function evalLoop(ast: AstNode, env: Env): AstNode {
    while (true) {
      //console.log("EVAL:", ast)
      if (!Array.isArray(ast)) return evalOrBind(ast, env);

      // apply
      ast = macroExpand(ast, env);
      if (!Array.isArray(ast)) return evalOrBind(ast, env);

      switch (ast[0]) {
        // update current environment
        case "def":
          assert(typeof ast[1] === "string");
          return setEnv(env, ast[1], evalLoop(ast[2], env));

        // define new function (lambda)
        case "fn": {
          const f = (...a: any[]) =>
            evalLoop(at(ast, 2), evalOrBind(at(ast, 1), env, a));

          f["A"] = [ast[2], env, ast[1]];
          return f as any;
        }

        // new environment with bindings
        case "let": {
          if (ast.length < 2) throw fatal(`let: expected array len=2: ${ast}`);
          if (!Array.isArray(ast[1]))
            throw fatal(`let: expected array at [1]: ${ast}`);

          env = Object.create(env);
          for (let i = 0; i < ast[1]?.length; i++) {
            if (i % 2) {
              env[symbolAt(ast, 1, i - 1)] = evalLoop(symbolAt(ast, 1, i), env);
            }
          }
          ast = ast[2];
          break;
        }

        // quote (unevaluated)
        case "`":
          return ast[1];

        // branching conditional
        case "if":
          ast = evalLoop(ast[1], env) ? ast[2] : ast[3];
          break;

        // multiple forms (for side-effects)
        case "do":
          evalOrBind(ast.slice(1, ast.length - 1), env);
          ast = ast[ast.length - 1];
          break;

        // mark as macro
        case "~": {
          const f = evalLoop(ast[1], env); // eval regular function
          (f as any)["M"] = 1; // mark as macro
          return f;
        }

        // get or set attribute
        case ".-": {
          const el = evalOrBind(ast.slice(1), env),
            x = el[0][el[1]];
          return 2 in el ? (el[0][el[1]] = el[2]) : x;
        }

        // call object method
        case ".": {
          const el = evalOrBind(ast.slice(1), env),
            x = el[0][el[1]];
          return x.apply(el[0], el.slice(2));
        }

        // try/catch
        case "try":
          try {
            return evalLoop(ast[1], env);
          } catch (e) {
            return evalLoop(
              at(ast, 2, 2),
              evalOrBind([at(ast, 2, 2)], env, [e])
            );
          }

        // invoke list form
        default: {
          const el = evalOrBind(ast, env),
            f = el[0];
          if (f.A) {
            ast = f.A[0];
            env = evalOrBind(f.A[2], f.A[1], el.slice(1));
          } else {
            return f(...el.slice(1));
          }
        }
      }
    }
  }

  E = Object.assign(Object.create(E), {
    // Core
    js: eval,
    eval: (...a: AstNode[]) => evalLoop(a[0], E),
    assert,
    throw: (...a: AstNode[]) => {
      throw a[0];
    },

    // Console
    info: (msg: any, ...a: any[]) => log.info(msg, ...a),
    print: (...a: any[]) => console.log(...a),
    printf,
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

    "#": (...a: AstNode[]) => 'really this is obj?',

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
    "obj/entries": (o: object) => Object.entries(o),
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
    "rpc/new": 'tbd',
    "rpc/send": 'tbd',
    "rpc/recv": 'tbd',
    
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
  ${prog} start [<file>...]            Start jsonrpc server
  ${prog} eval <expr>...               Evaluate expression from argv
  ${prog} fmt <file>... [-o=<format>]  Format the code 
  ${prog} -h | --help                  Show usage
  ${prog} -v | --version               Show version

Options:
  -h --help           Show this screen.
  -o --output-format  Any of: yaml, json, lisp.
  --version           Show version.

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
      const code = JSON.parse(await Deno.readTextFile(file));
      for (const expr of code) {
        m.eval(expr);
      }
    }
  }
}
