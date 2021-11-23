import {
  DEFAULT_SCHEMA,
  parse,
  Type,
} from "yaml";

import yaml2 from 'https://esm.sh/yaml?dev'

import * as tw from 'https://esm.sh/tailwindcss'
import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";

console.log(tw)


const StrType = new Type("!str", {
  kind: "scalar",
  construct: (data: string) => {
    return ["str", data];
  }
  /* other type options here*/
});
const schema = DEFAULT_SCHEMA.extend({ explicit: [StrType] });

export function toJson(yamlStr: string) {

  // const ast = yaml2.parse(yamlStr)
  const cst = yaml2.parseCST(yamlStr)
  const doc= yaml2.parseDocument(yamlStr)
  
  console.log(yaml2, Object.keys(yaml2), Object.keys(doc));
  
  for (const item of doc.contents.items) {
    console.log(item, item.constructor.name)
  }
  
  // yaml2.visit(doc, {
  //   // Pair(_, pair) {
  //   //   if (pair.key && pair.key.value === '3') return YAML.visit.REMOVE
  //   // },
  //   Scalar(key: any, node: any) {
  //     console.log('Scalar', key, node)
  //     if (
  //       key !== 'key' &&
  //       typeof node.value === 'string' &&
  //       node.type === 'PLAIN'
  //     ) {
  //       node.type = 'QUOTE_SINGLE'
  //     }
  //   }
  // })
  
  // console.log(yaml2.stringify(doc.contents.items));
  
  debugger;
  assert(false)
  // console.log(typeof doc.contents)
  // assert(typeof doc.contents)
  // Deno.inspect(ast)

  return parse(yamlStr, { schema });
}