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

export function assertArray(arg: any): asserts arg is any[] {
  assert(isArray(arg));
}

// export class KindError extends Error {

export class KindError extends Error {
  objects: any[];
  constructor(...objects: any[]) {
    super();
    this.objects = objects;
  }
  
  get message() {
    return this.pretty()
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
