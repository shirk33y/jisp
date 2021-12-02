import * as log from "https://deno.land/std/log/mod.ts";
import {
  info,
  error,
  warning,
  critical,
  debug,
} from "https://deno.land/std/log/mod.ts";
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
import { LevelName, HandlerOptions } from "https://deno.land/std/log/mod.ts";
import { isString } from "/utils.ts";

export class TerminalHandler extends log.handlers.BaseHandler {
  constructor(levelName: LevelName, options: HandlerOptions = {}) {
    super(levelName, options);
  }

  log(msg: string): void {
    Deno.stderr.write(new TextEncoder().encode(msg));
    Deno.stderr.write(new TextEncoder().encode("\n"));
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
    italic(brightBlack(name === "default" ? "main" : name)),
    dim(gray("]")),
  ].join("");

export const prettyFormatter = (rec: log.LogRecord) => {
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

export const jsonFormatter = (rec: log.LogRecord) =>
  JSON.stringify([
    rec.datetime,
    rec.level,
    rec.loggerName,
    rec.msg,
    ...rec.args,
  ]);

export const defaultSetup = async (level: LevelName) => {
  await log.setup({
    handlers: {
      console: new TerminalHandler(level, {
        formatter: prettyFormatter,
      }),
      // file: new log.handlers.RotatingFileHandler("INFO", {
      //   filename: "./var/a.log",
      //   maxBytes: 1500000,
      //   maxBackupCount: 5,
      //   formatter: jsonFormatter,
      // }),
    },
    loggers: {
      default: {
        level: "DEBUG",
        handlers: ["console"],
        // handlers: ["console", "file"],
      },
      eval: {
        level: "DEBUG",
        handlers: ["console"],
        // handlers: ["console", "file"],
      },
    },
  });
};

await defaultSetup('ERROR')

export type { LevelName };

export { log, debug, info, error, warning, critical };
