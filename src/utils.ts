import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";
import * as log from "log";
import * as yaml from "yaml";

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

export const isFunction = (v: any) => typeof v === "function";

export const isPromise = (v: any) =>
  typeof v === "object" && typeof v.then === "function";

export function assertArray(arg: any): asserts arg is any[] {
  assert(isArray(arg));
}

// console.log(c`red ${c`green ${'blue'.bold}.blue`}.green`.red);
export function toSpacez(ast: any, { maxLines = 3, colors = true }) {


}





// if(isDebug && window.console && console.log && console.warn && console.error){
//   window.debug = {
//       'log': window.console.log,
//       'warn': window.console.warn,
//       'error': window.console.error
//   };
// }else{
//   window.debug = {
//       'log': function(){},
//       'warn': function(){},
//       'error': function(){}
//   };
// }

// export class KindError extends Error {

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
