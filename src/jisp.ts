// import * as log from "log";
import * as _ from "lodash";
import { AstNode, Env } from "/ast.ts";
import {
  arythmetics,
  constructors,
  log,
  platform,
  list,
  obj,
  str,
  num,
} from "./std.ts";
import { evalAst } from "./eval.ts";
import * as std from "./std.ts";

export function jisp(env: Env) {
  env = Object.assign(Object.create(env), {
    ...arythmetics,
    ...constructors,
    ...platform.deno,
    std,
    // list,
    // obj,
    // str,
    // num,
    obj: obj.new,
    "obj.cat": obj.cat,
    "obj.merge": obj.cat,
    // list: list.new,
    "list.map": async (l: any[], f: any) => await Promise.all(l.map(f)),
    // 'list.map': Array.prototype.map,
    "list.append": list.append,
    "str.is": str.is,
    "str.fmt": str.fmt,
    "str.join": str.join,
    "str.cat": str.cat,
    list: (...a: any[]) => a,
    $$path: [],
    eval: (a: AstNode) => evalAst(a, env),
    .../* extra env */ {},
  });
  // console.log(env);

  return env;
}
