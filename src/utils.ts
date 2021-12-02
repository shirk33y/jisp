import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";
import * as log from "/log.ts";
import * as yaml from "yaml2";
import * as _ from "lodash";

export function die(code: number, msg: any, ...args: any[]) : never {
  log.critical(msg, ...args);
  //TODO: is this best solution?
  Deno.stdout.writeSync(new TextEncoder().encode("\n"));
  Deno.exit(code);
}

export function isArray(arg: any): arg is any[] {
  return Array.isArray(arg);
}

export function isString(arg: any): arg is string {
  return typeof arg === "string";
}

export function isNumber(arg: any): arg is number {
  return typeof arg === "number";
}

export const isFunction = (v: any) => typeof v === "function";

export const isPromise = (v: any) =>
  typeof v === "object" && typeof v.then === "function";

// export const isDate = (d: any) => _.isDate(d);
export const isDate = (d: any) => d instanceof Date && !isNaN(d.valueOf());

export function assertArray(arg: any): asserts arg is any[] {
  assert(isArray(arg));
}

export function toSpacez(ast: any, { maxLines = 3, colors = true }) {}

export function stdoutWrite(value: any) {
  return Deno.stdout.write(new TextEncoder().encode(value));
}

export function stderrWrite(value: any) {
  return Deno.stderr.write(new TextEncoder().encode(value));
}

export class KindError extends Error {
  objects: any[];
  constructor(...objects: any[]) {
    super();
    this.objects = objects;
  }

  get message() {
    return this.pretty();
  }

  pretty() {
    return `${this.constructor.name}: ${this.objects
      .map((o: any) => yaml.stringify(o))
      .join(", ")}`;
  }
}

export class PromiseError extends KindError {}
export class UndefinedSymbolError extends KindError {}
export class IoError extends KindError {}
export class PlatformError extends KindError {}
