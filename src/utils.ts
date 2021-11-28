import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";
import * as log from "log";
import * as yaml from "yaml2";
import * as _ from "lodash";

export function fatal(...msg: any[]) {
  log.error("fatal: ", ...msg);

  return new Error(msg.map((m) => `${m}`).join(" "));
}

export function isArray(arg: any): arg is any[] {
  return Array.isArray(arg);
}

export function isString(arg: any): arg is string {
  return typeof arg === "string";
}

export function isNumber(arg: any): arg is number {
  // return _.isFinite(arg);
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

// const digits = (num: number, padWidth = 2) => {
//   const width = Math.floor(Math.log10(num) + 1);
//   const pad = new Array(padWidth - width).fill("0").join("");

//   return `${dim(gray(pad))}${dim(white(num.toString()))}`;
// };

// const padStr = (str: string, width: number, padStr = " ") => {
//   const trunc = str.substr(0, width);
//   return trunc + new Array(width - trunc.length).fill(padStr).join("");
// };

// const date = (date: Date) => {
//   const colon = dim(gray(":"));

//   return [
//     digits(date.getHours()),
//     colon,
//     digits(date.getMinutes()),
//     colon,
//     digits(date.getSeconds()),
//   ].join("");
// };

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
