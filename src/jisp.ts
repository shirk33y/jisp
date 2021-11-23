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
    obj,
    str,
    num,
    eval: (a: AstNode) => evalAst(a, env),
    .../* extra env */ {},
  });
  // console.log(env);

  return env;
}
