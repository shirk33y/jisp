import * as yaml from "yaml2";

export function toMinimal(yamlStr: string) {
  return yaml.parse(yamlStr);
}
