import { tabtab, ParsedEnv } from "https://deno.land/x/tabtab@0.2.2/mod.ts";
import { parse } from "https://deno.land/std@0.113.0/flags/mod.ts";

const opts = parse(Deno.args, {
  string: ["foo", "bar"],
  boolean: ["help", "version", "loglevel"],
});

const args = opts._;
const completion = (env: ParsedEnv) => {
  if (!env.complete) return;

  // Write your completions there

  if (env.prev === "foo") {
    return tabtab.log(["is", "this", "the", "real", "life"]);
  }

  if (env.prev === "bar") {
    return tabtab.log(["is", "this", "just", "fantasy"]);
  }

  if (env.prev === "--loglevel") {
    return tabtab.log(["error", "warn", "info", "notice", "verbose"]);
  }

  return tabtab.log([
    "--help",
    "--version",
    "--loglevel",
    "foo",
    "bar",
    "someCommand:a comprehensive description of the command",
    {
      name: "someOtherCommand",
      description: "comprehensive description of the other command",
    },
    "anotherOne",
  ]);
};

const run = async () => {
  const cmd = args[0];

  // Write your CLI there

  // Here we install for the program `tabtab-test` (this file), with
  // completer being the same program. Sometimes, you want to complete
  // another program that's where the `completer` option might come handy.
  if (cmd === "install-completion") {
    await tabtab.install({
      name: "tabtab-test",
      completer: "tabtab-test",
      location: tabtab.defaultLocation(),
    });

    return;
  }

  if (cmd === "uninstall-completion") {
    // Here we uninstall for the program `tabtab-test` (this file).
    await tabtab.uninstall({
      name: "tabtab-test",
    });

    return;
  }

  // The completion command is added automatically by tabtab when the program
  // is completed. Can be configured with the `cmd` option in install.
  if (cmd === "completion") {
    const env = tabtab.parseEnv();
    return completion(env);
  }
};

run();