import * as yaml from "yaml2";

export function toMinimal(yamlStr: string) {
  return yaml.parse(yamlStr);
}

export function fromMinimal(jsonStr: string) {
  return yaml.stringify(jsonStr);
}