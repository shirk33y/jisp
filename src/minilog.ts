/*
 * Copyright 2016, Matthieu Dumas
 * This work is licensed under the Creative Commons Attribution 4.0 International License.
 * To view a copy of this license, visit http://creativecommons.org/licenses/by/4.0/
 */

/* Usage :
 * var log = Logger.get("myModule") // .level(Logger.ALL) implicit
 * log.info("always a string as first argument", then, other, stuff)
 * log.level(Logger.WARN) // or ALL, DEBUG, INFO, WARN, ERROR, OFF
 * log.debug("does not show")
 * log("but this does because direct call on logger is not filtered by level")
 */
export const logger = (function () {
  const levels = {
    ALL: 100,
    DEBUG: 100,
    INFO: 200,
    WARN: 300,
    ERROR: 400,
    OFF: 500,
  };
  const cache: Record<any, any> = {};
  const cons = window.console;
  const noop = function () {};
  const level = function (this: any, level: any) {
    this.error =
      level <= levels.ERROR
        ? cons.error.bind(cons, "[" + this.id + "] - ERROR - %s")
        : noop;
    this.warn =
      level <= levels.WARN
        ? cons.warn.bind(cons, "[" + this.id + "] - WARN - %s")
        : noop;
    this.info =
      level <= levels.INFO
        ? cons.info.bind(cons, "[" + this.id + "] - INFO - %s")
        : noop;
    this.debug =
      level <= levels.DEBUG
        ? cons.log.bind(cons, "[" + this.id + "] - DEBUG - %s")
        : noop;
    this.log = cons.log.bind(cons, "[" + this.id + "] %s");
    return this;
  };
  (levels as any).get = function (id: any) {
    let res = cache[id];
    if (!res) {
      let ctx: any = { id, level }; // create a context
      ctx.level(log.ALL); // apply level
      res = ctx.log; // extract the log function, copy context to it and returns it
      for (const prop in ctx) {
        res[prop] = ctx[prop];
      }
      cache[id] = res;
    }
    return res;
  };
  return levels; // return levels augmented with "get"
})();

export const log = (logger as any).get(import.meta.url);
