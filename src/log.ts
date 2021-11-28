import * as log from "https://deno.land/std/log/mod.ts";
import * as YAML from "yaml2";
import * as _ from "lodash";
import {
  bold,
  gray,
  dim,
  italic,
  brightBlue,
  red,
  brightBlack,
  brightRed,
  yellow,
  white,
} from "https://deno.land/std@0.106.0/fmt/colors.ts";
import type {
  LevelName,
  HandlerOptions,
} from "https://deno.land/std/log/mod.ts";
import { isString } from "/utils.ts";

class TerminalHandler extends log.handlers.BaseHandler {
  constructor(levelName: LevelName, options: HandlerOptions = {}) {
    super(levelName, options);
    this.log = console.log.bind(console);
  }
}

const LEVEL_COLORS = {
  NOTSET: brightBlack,
  DEBUG: brightBlue,
  INFO: white,
  WARNING: yellow,
  ERROR: red,
  CRITICAL: brightRed,
};

type Paint = (str: string) => string;

const getLevelPaint = (levelName: string): Paint => {
  return LEVEL_COLORS[levelName as LevelName];
};

const level = (levelName: string, levelPaint: Paint) => {
  return bold(levelPaint(levelName.substr(0, 1).toUpperCase()));
};

const sep = (sep = " | ") => {
  return dim(gray(sep));
};

const inspect = (obj: any) =>
  Deno.inspect(obj, {
    compact: true,
    colors: true,
    depth: 2,
  });

const prefixLines = (text: string, prefix: string) =>
  text
    .split(/\n/)
    .map((line, i) => (i === 0 ? line : `${prefix}${line}`))
    .join("\n");

const msg = (msg: any) => {
  // const data = YAML.parse(msg); 
  return bold(anything(msg));
};

const anything = (data: any) => {
  if (isString(data)) {
    return data;
  }

  return inspect(data).trim();
};

const args = (args: any[]) => {
  if (args.length === 0) return "";

  const str = args.map((arg) => sep(", ") + anything(arg)).join("");

  return prefixLines(str, "  ");
};

const loggerName = (name: string) =>
  [
    dim(gray("[")),
    italic(brightBlack(name === "default" ? "" : name)),
    dim(gray("]")),
  ].join("");

const prettyFormatter = (rec: log.LogRecord) => {
  const levelPaint = getLevelPaint(rec.levelName) as any;

  return [
    level(rec.levelName, levelPaint),
    " ",
    loggerName(rec.loggerName),
    " ",
    msg(rec.msg),
    args(rec.args),
  ].join("");
};

const jsonFormatter = (rec: log.LogRecord) =>
  JSON.stringify([
    rec.datetime,
    rec.level,
    rec.loggerName,
    rec.msg,
    ...rec.args,
  ]);

await log.setup({
  handlers: {
    console: new TerminalHandler("DEBUG", {
      formatter: prettyFormatter,
    }),
    file: new log.handlers.RotatingFileHandler("INFO", {
      filename: "./var/a.log",
      maxBytes: 1500000,
      maxBackupCount: 5,
      formatter: jsonFormatter,
    }),
  },
  loggers: {
    default: {
      level: "DEBUG",
      handlers: ["console", "file"],
    },
    eval: {
      level: "DEBUG",
      handlers: ["console", "file"],
    },
  },
});

export const logger = log.getLogger;

export default log 
