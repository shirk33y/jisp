import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";
import { AstNode, Env, Exprs, Fn } from "./ast.ts";
import * as _ from "lodash";
// import * as _ from "https://deno.land/x/lodash@4.17.19/lodash.js";
import {
  assertArray,
  isArray,
  isString,
  isPromise,
  isFunction,
  UndefinedSymbolError,
} from "./utils.ts";

import { logger } from "/log.ts";

const log = logger("eval");

log.error(_)

export async function resolve(ast: AstNode, env: Env) {
  // list?
  if (isArray(ast)) {
    return await Promise.all(ast.map((a: AstNode) => evalAst(a, env)));
  }
  // symbol?
  if (isString(ast)) {
    return getEnv(env, ast);
  }

  return await ast; // ast unchanged
}

// Return new Env with symbols in ast bound to
// corresponding values in exprs
//
// fn [ast ... rest]
//
// [fn expr1 expr2]
//
export async function bindFn(ast: AstNode, env: Env, exprs: Exprs) {
  assertArray(ast);
  env = Object.create(env);

  for (let i = 0; i < ast.length; i++) {
    if (ast[i] === "...") {
      await setEnv(env, ast[i + 1] as string, exprs.slice(i));
      break;
    }
    await setEnv(env, ast[i] as string, exprs[i]);
  }

  return env;
}

export function getEnv(env: Env, key: string, default_?: any): any {
  if (key in env) return env[key];
  if (default_ !== undefined) return default_;

  // console.log(Deno.inspect(env).substr(0, 100));
  // console.table(env);

  throw new UndefinedSymbolError(key);
  // env.throw(new UndefinedSymbolError(key));
}

export async function setEnv(env: Env, key: string, value: any): Promise<Env> {
  const resolved = await value;
  
  log.debug('set', key, value)

  // if (isPromise(value)) {
  //   console.error("promise:", value, "resolved:", resolved);
  // }

  // if (isFunction(value) && "_BIND" in value) {
  //   console.log(" -> ", key, "BIND", value._BIND?.[0]);
  // } else {
  //   console.log(" +  ", key, " =  ", typeof value, '   ', value);
  // }
  // console.log('%c + %c%s %c= %c%s', 'color: orange', 'color: green', key, 'color: gray', 'color: white', value)
  // console.log('%c%s%c: %c%s')
  return (env[key] = resolved);
}

export function hasEnv(env: Env, key: string): boolean {
  return key in env;
}

export async function macroExpand(ast: AstNode, env: Env) {
  assertArray(ast);
  const [symbol, ...args] = ast;
  assert(typeof symbol === "string");

  while (hasEnv(env, symbol) && getEnv(env, symbol)._MACRO) {
    ast = await getEnv(env, symbol)(args);
  }
  return ast;
}

export async function evalAst(ast: AstNode, env: Env): Promise<AstNode> {
  while (true) {
    log.info("eval", ast);
    // console.debug('keys', Object.keys(env));
    if (!isArray(ast)) return await resolve(ast, env);

    // apply
    ast = await macroExpand(ast, env);
    if (!isArray(ast)) return await resolve(ast, env);

    const [fname, ...fargs] = ast;

    switch (fname) {
      // def my-symbol 42
      case "def": {
        const [name, value] = fargs;
        assert(typeof name === "string");

        return await setEnv(env, name, await evalAst(value, env));
      }

      // fn [arg1 arg2 ... rest]
      //   body
      case "fn": {
        const [args, body] = fargs;

        assert(isArray(args));

        const wrapperFn: Fn = async (...a: any[]) =>
          await evalAst(body, await bindFn(args, env, a));

        wrapperFn._BIND = [body, env, args];

        return wrapperFn;
      }

      // let [foo 1 bar 2]
      //   body
      case "let": {
        const [pairs, body] = fargs;
        assert(isArray(pairs));

        env = Object.create(env);

        for (let i = 0; i < pairs.length; i++) {
          if (i % 2) {
            const key = pairs[i - 1];
            assert(typeof key === "string");

            const value = pairs[i];

            await setEnv(env, key, await evalAst(value, env));
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
      case "if": {
        const [condition, yes, no] = fargs;
        ast = (await evalAst(condition, env)) ? yes : no;

        break;
      }

      // multiple forms (for side-effects)
      //
      // do
      //   fun1 1 2
      //   fun2 "foo"
      //
      case "do": {
        const rest = [...fargs];
        const last = rest.pop();

        assert(last);

        await resolve(rest, env);
        ast = last;

        break;
      }

      // quote (unevaluated)
      case "str":
      case "`":
        return fargs[0];

      // mark as macro
      case "macro":
      case "~": {
        const wrapperFn: Fn = (await evalAst(fargs[0], env)) as Fn; // eval regular function
        wrapperFn._MACRO = 1; // mark as macro

        return wrapperFn;
      }

      // get  attribute
      case "get":
      case ".": {
        const [object, ...path] = await resolve(fargs, env);

        return _.get(object, path);
      }

      // set attribute
      case "set": {
        const path = [...(await resolve(fargs, env))];
        const object = path.shift();
        const value = path.pop();

        return _.set(object, path, value);
      }

      // call object method
      case "call":
      case "->": {
        const [object, method, ...args] = await resolve(fargs, env);

        return await object[method].apply(object, args);
      }

      // try/catch
      // case "try":
      //   try {
      //     return await evalAst(fargs[0], env);
      //   } catch (err) {
      //     assert(isArray(a2));
      //     return await evalAst(a2[2], await bindFn([a2[2]], env, [err]));
      //   }

      // invoke list form
      default: {
        const [fn, ...args] = await resolve(ast, env);

        if (fn._BIND) {
          const [body, bindEnv, argDef] = fn._BIND;
          ast = body;
          env = await bindFn(argDef, bindEnv, args);
        } else {
          return await fn(...args);
        }
      }
    }
  }
}
