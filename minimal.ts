import * as log from "https://deno.land/std@0.113.0/log/mod.ts";
import {printf, sprintf} from "https://deno.land/std@0.113.0/fmt/printf.ts";
import docopt from "https://cdn.deno.land/docopt/versions/v1.0.1/raw/dist/docopt.mjs";
// import _ from "https://deno.land/x/lodash@4.17.19/dist/lodash.js";
import * as _ from "https://deno.land/x/lodash@4.17.19/dist/lodash.js";
import {assert} from "https://deno.land/std@0.113.0/testing/asserts.ts";

type Scalar = string | boolean | number | null;
type SaneScalar = string | boolean | number | null;

// type Atom = string | boolean | number

interface Ast extends Array<Ast | Scalar> {}

type AstNode = Scalar | Ast;

interface Env {
  [bind: string]: any;
}

type Exprs = Array<any>;

// interface ItemArea {
//     [n: number]: {
//         name: string
//     } | ItemArea;
// };
function fatal(...msg: any[]) {
  log.error("fatal: ", ...msg);

  return new Error(msg.map((m) => `${m}`).join(" "));
}

function at(ast: any, ...[index, ...next]: number[]): any {
  // const [index, ...nextPath] = path
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

function sym(ast: AstNode) {}

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
    // if (ast && typeof ast[0] !== 'string')
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

      //   const [a0, a1, a2, a21, a22] =
      switch (ast[0]) {
        // update current environment
        case "def":
          assert(typeof ast[1] === "string");
          return setEnv(env, ast[1], evalLoop(ast[2], env));

        // mark as macro
        case "~": {
          const f = evalLoop(ast[1], env); // eval regular function
          (f as any)["M"] = 1; // mark as macro
          return f;
        }

        // quote (unevaluated)
        case "`":
          return ast[1];

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

        // multiple forms (for side-effects)
        case "do":
          evalOrBind(ast.slice(1, ast.length - 1), env);
          ast = ast[ast.length - 1];
          break;

        // branching conditional
        case "if":
          ast = evalLoop(ast[1], env) ? ast[2] : ast[3];
          break;

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
    js: eval,
    eval: (...a: AstNode[]) => evalLoop(a[0], E),
    // TODO: figure out why global doesn't have this when non-interactive
    //"require":     require,
    info: (msg: any, ...a: any[]) => log.info(msg, ...a),
    print: (...a: any[]) => console.log(...a),
    printf,
    sprintf,
    assert,
    log: (...a: any[]) => console.log(...a),

    // These could all also be interop
    "==": (a: number, b: number) => a === b,
    "<": (a: number, b: number) => a < b,
    ">": (a: number, b: number) => a > b,
    "+": (a: number, b: number) => a + b,
    "-": (a: number, b: number) => a - b,
    "*": (a: number, b: number) => a * b,
    "**": (a: number, b: number) => Math.pow(a, b),
    "/": (a: number, b: number) => a / b,
    "//": (a: number, b: number) => Math.floor(a / b),
    str: (a: any) => a?.toString() || "",
    int: (a: any) => parseInt(a, 10),
    float: (a: any) => parseFloat(a),
    list: (a: any) => Array.from(a),
    obj: (...a: AstNode[]) => _.fromEntries(),
    "is-int": (n: any) => n === +n && n === (n | 0),
    "is-float": (n: any) => n === +n && n !== (n | 0),
    "is-str": (a: any) => typeof a === "string",
    "is-list": (a: any) => Array.isArray(a),

    // isa: (...a: ScalarOrAst[]) => a[0] instanceof a[1],
    // type: (...a: ScalarOrAst[]) => typeof a[0],
    // new: (...a: AstNode[]) => new (a[0].bind(...a))(),
    // del: (...a: ScalarOrAst[]) => delete a[0][a[1]],
    // "list":  (...a: AstNode[]) => a,
    //"map":   (...a: ScalarOrAst[]) => a[1].map(x => a[0](x)),
    throw: (...a: AstNode[]) => {
      throw a[0];
    },
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
  ${prog} eval <expr>...
  ${prog} run <file>...
  ${prog} fmt <file>... [-o=<format>]
  ${prog} -h | --help
  ${prog} --version

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
