import { sprintf } from "https://deno.land/std@0.114.0/fmt/printf.ts";
import * as _ from "lodash";
import {
  assert,
  AssertionError,
} from "https://deno.land/std@0.114.0/testing/asserts.ts";
import { AstNode } from "/ast.ts";

// Arithmetics
export const arythmetics = {
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
};

// Function ops (not sure if needed)
export const fn_ = {
  new: () => null,
  bind: () => null,
  partial: () => null,
  curry: () => null,
  await: () => null,
  map: () => null,
  unwind: () => null,
  wrap: () => null,
  compose: () => null,
};

export const num = {
  new: (value?: any) => parseFloat(value),
  complex: [3, 14],
  matrix: [3, 14, 15],
  pi: 3.14,
  is: (value?: any) => typeof value === "number",
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
  ...arythmetics,
};

// Type checkers
export const is = {
  fn: (a: any) => typeof a === "function",
  int: (n: any) => n === +n && n === (n | 0),
  float: (n: any) => n === +n && n !== (n | 0),
  str: (a: any) => typeof a === "string",
  list: (a: any) => Array.isArray(a),
  obj: (a: any) => typeof a === "object" && !Array.isArray(a) && a !== null,
};

// String ops
export const str = {
  new: (s: any = "") => `${s}`,
  is: (s: any) => typeof s === "string",
  cat: (...s: string[]) => s.join(""),
  fmt: (f: string, ...a: any[]) => sprintf(f, ...a),
  join: (delim: string, ...arr: string[]) => arr.join(delim),
  len: (f: string) => f.length,
  upper: (f: string) => f.toUpperCase(),
  lower: (f: string) => f.toLowerCase(),
  "to-json": (s: string) => JSON.stringify(s),
};

// List ops
export const list = {
  new: (...items: any[]) => items,
  map: (b: Array<any>, a: (...args: any) => any) => b.map((x) => a(x)),
  filter: (a: (...args: any) => any, b: Array<any>) => b.filter((x) => a(x)),
  some: (a: (...args: any) => any, b: Array<any>) => b.some((x) => a(x)),
  every: (a: (...args: any) => any, b: Array<any>) => b.every((x) => a(x)),
  has: (l: any[], item: any) => l.includes(item),
  cat: (l: any[], ...c: any[][]) => l.concat(...c),
  // append: (l: any[], c: any) => [...l, c],
  // MUTABLE!
  append: (l: any[], c: any) => l.push(c),
  prepend: (l: any[], c: any) => l.unshift(c),
  "to-json": (l: any[]) => JSON.stringify(l),
};

//  "#": (...a: AstNode[]) => "really this is obj?",

// Object ops
export const obj = {
  new: (...a: AstNode[]) => {
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
  is: (a: any) => typeof a === "object" && !Array.isArray(a) && a !== null,
  items: (o: object) => Object.entries(o),
  get: (o: object, ...p: string[]) => _.get(o, p),
  keys: (o: object) => Object.keys(o),
  values: (o: object) => Object.values(o),
  has: (o: object, k: any) => k in o,
  cat: (o: object, ...c: object[]) => Object.assign(o, ...c),
  "to-json": (o: object) => JSON.stringify(o),
};

// Date ops
export const time = {
  new: (ts: number) => new Date(ts),
  "date/from-str": (s: string, f: string) => {
    throw "tbd";
  },
  is: (d: any) => d instanceof Date,
  "date/+": (d: Date, i: number, p: string) => {
    throw "tbd";
  },
  "date/-": (d: Date, i: number, p: string) => {
    throw "tbd";
  },
  fmt: (d: Date, f: string) => {
    throw "tbd";
  },
};

export const log = console;

export const err = {
  new: (message: string) => new Error(message),
  is: (arg: any) => arg instanceof Error,
  "to-json": (e: Error) => e.toString(),
  assert: (expr: unknown, msg = ""): asserts expr => {
    if (expr) throw new AssertionError(msg);
  },
  throw: (e: Error) => {
    throw e;
  },
};

// File ops
export const file = {
  new: () => null,
  read: () => null,
  "read-line": () => null,
  open: () => null,
  exists: () => null,
  close: () => null,
  write: () => null,
  delete: () => null,
  touch: () => null,
  move: () => null,
};

// Path ops
export const path = {
  new: () => null,
  base: () => null,
  dir: () => null,
  rel: () => null,
  abs: () => null,
  join: () => null,
};

// Url ops
// in fact url = file/* + path/*
export const url = {
  new: () => null,
  "from-path": () => null,
  base: () => null,
  dir: () => null,
  join: () => null,
  host: () => null,
  port: () => null,
  scheme: () => null,
  path: () => null,
  query: () => null,
  param: () => null,
  params: () => null,
  fragment: () => null,
  fetch: () => null,
  head: () => null,
  get: () => null,
  post: () => null,
  put: () => null,
  options: () => null,
  delete: () => null,
};

// RPC
export const rpc = {
  new: "tbd",
  send: "tbd",
  recv: "tbd",
};

// UI / Web Component
export const ui = {
  new: "tbd",
  render: "tbd",
  effect: "tbd",
  state: "tbd",
  dispatch: "tbd",
};

export const state = {
  new: "tbd",
  get: "tbd",
  dispatch: "tbd",
  reduce: "tbd",
  saga: "tbd",
  run: "tbd",
};

export const platform = {
  deno: {
    "eval-js": eval,
    import: (uri: string) => import(uri),
    throw: (err: any) => {
      throw err;
    },
    log: log.info,
    // log: async (m: any) => {
    //   return log.info(await Promise.all(m));
    // },
  },
  python: {
    eval: () => null,
    import: () => null,
    version: () => null,
  },
};

export const constructors = {
  str: str.new,
  num: num.new,
  list: list.new,
  obj: obj.new,
};
// Candidates
// isa: (...a: ScalarOrAst[]) => a[0] instanceof a[1],
// type: (...a: ScalarOrAst[]) => typeof a[0],
// new: (...a: AstNode[]) => new (a[0].bind(...a))(),
// del: (...a: ScalarOrAst[]) => delete a[0][a[1]],
// "list":  (...a: AstNode[]) => a,
// read: (...a: ScalarOrAst[]) => JSON.parse(a[0]),
// rep: (...a: ScalarOrAst[]) => JSON.stringify(EVAL(JSON.parse(a[0]), E)),
