import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";
import { AstNode, Env, Exprs, Fn } from "./ast.ts";
import * as yaml from "yaml";
import { log } from "./std.ts";
import {
  assertArray,
  isArray,
  isString,
  UndefinedSymbolError,
} from "./utils.ts";

export async function resolve(ast: AstNode, env: Env) {
  // list?
  if (isArray(ast)) {
    return await Promise.all(ast.map((a: AstNode) => evalAst(a, env)));
  }
  // symbol?
  if (isString(ast)) {
    return getEnv(env, ast);
  }

  return ast; // ast unchanged
}

// Return new Env with symbols in ast bound to
// corresponding values in exprs
//
// fn [ast ... rest]
//
// [fn expr1 expr2]
//
export function bindFn(ast: AstNode, env: Env, exprs: Exprs) {
  assertArray(ast);
  env = Object.create(env);

  ast?.some((a: any, i: number) =>
    a == "..."
      ? (env[ast[i + 1] as string] = exprs.slice(i))
      : ((env[a] = exprs[i]), 0)
  );
  return env;
}

export function getEnv(env: Env, key: string, default_?: any): any {
  if (key in env) return env[key];
  if (default_ !== undefined) return default_;

  env.throw(new UndefinedSymbolError(key));
}

export function setEnv(env: Env, key: string, value: any): Env {
  return (env[key] = value);
}

export async function macroExpand(ast: AstNode, env: Env) {
  assertArray(ast);
  const [a0, ...a$] = ast;
  assert(typeof a0 === "string");

  while (a0 in env && env[a0]._MACRO) {
    ast = await env[a0](a$);
  }
  return ast;
}

export async function evalAst(ast: AstNode, env: Env): Promise<AstNode> {
  while (true) {
    console.debug("eval", ast);
    // console.debug('keys', Object.keys(env));
    if (!isArray(ast)) return await resolve(ast, env);

    // apply
    ast = await macroExpand(ast, env);
    if (!isArray(ast)) return await resolve(ast, env);

    const [fname, ...fargs] = ast;
    const [a1, a2, a3] = fargs;

    switch (fname) {
      // def my-symbol 42
      case "def": {
        const [name, value] = fargs;
        assert(typeof name === "string");

        return setEnv(env, name, await evalAst(value, env));
      }

      // fn [arg1 arg2 ... rest]
      //   body
      case "fn": {
        const [args, body] = fargs;
        assert(isArray(args));

        const func: Fn = (...a: any[]) => evalAst(body, bindFn(args, env, a));
        func._BIND = [body, env, args];

        return func;
      }

      // let [foo 1 bar 2]
      //   body
      case "let": {
        const [pairs, body] = fargs;
        assert(isArray(pairs));

        env = Object.create(env);

        for (let i = 0; i < pairs.length; i++) {
          if (i % 2) {
            const k = pairs[i - 1];
            assert(typeof k === "string");
            const v = pairs[i];
            env[k] = await evalAst(v, env);
          }
        }
        ast = body;
        break;
      }

      // if [= foo 123]
      //   print "foo is 123"
      //   do
      //     print "foo is not 123"
      //     print "so, i'm mad."
      //
      case "if":
        ast = (await evalAst(a1, env)) ? a2 : a3;
        break;

      // multiple forms (for side-effects)
      //
      // do
      //   fun1 1 2
      //   fun2 "foo"
      //
      case "do":
        await resolve(ast.slice(1, ast.length - 1), env);
        ast = ast[ast.length - 1];
        break;

      // quote (unevaluated)
      case "str":
      case "`":
        return a1;

      // mark as macro
      case "~": {
        const fn: Fn = (await evalAst(a1, env)) as Fn; // eval regular function
        fn._MACRO = 1; // mark as macro
        return fn;
      }

      // get or set attribute
      case ".-": {
        const [e0, e1, e2] = await resolve(ast.slice(1), env);

        return e2 !== undefined
          ? (e0[e1] = e2) // set
          : e0[e1]; // get;
      }

      // call object method
      case ".": {
        const [e0, e1, ...e$] = await resolve(ast.slice(1), env);

        return e0[e1].apply(e0, e$);
      }

      // case "import": {

      // }
      // try/catch
      case "try":
        try {
          return await evalAst(a1, env);
        } catch (err) {
          assert(isArray(a2));
          return await evalAst(a2[2], bindFn([a2[2]], env, [err]));
        }

      // invoke list form
      default: {
        // console.log('ast', ast)
        const [fn, ...args] = await resolve(ast, env);
        // console.log('fn', fn, args)

        if (fn._BIND) {
          const [body, bindEnv, argDef] = fn._BIND;
          ast = body;
          env = bindFn(argDef, bindEnv, args);
        } else {
          return await fn(...args);
        }
      }
    }
  }
}
