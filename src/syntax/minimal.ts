import * as yaml from "yaml2";

// import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";

export function parse(yamlStr: string) {
  return yaml.parse(yamlStr);
}
