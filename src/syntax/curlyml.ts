// import {
//   DEFAULT_SCHEMA,
//   parse,
//   Type,
// } from "https://esm.sh/yaml?dev";

import * as yaml2 from "yaml2";
// import {createSeq} from 'yaml2/schema/common/seq.ts'
import { createNode } from "yaml2/doc/createNode.ts";

import * as tw from "https://esm.sh/tailwindcss";
import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";

// console.log(tw, yaml2)

// const {parseCST, parseDocument} = yaml2

// const StrType = new Type("!str", {
//   kind: "scalar",
//   construct: (data: string) => {
//     return ["str", data];
//   }
//   /* other type options here*/
// });
// const schema = DEFAULT_SCHEMA.extend({ explicit: [StrType] });

export function parse(yamlStr: string) {
  // const ast = yaml2.parse(yamlStr)
  // const cst = parseCST(yamlStr)
  const doc = yaml2.parseDocument(yamlStr);

  // console.log(Object.keys(doc));

  // for (const item of doc.contents.items) {
  //   console.log(item, item.constructor.name)
  // }

  yaml2.visit(doc, {
    // Collection(...a) {
    //   console.log('a', a)
    //   return undefined
    // },
    // Pair(_, pair) {
    // console.log('Pair', pair)
    // if (pair.key && pair.key.value === '3') return YAML.visit.REMOVE
    // },
    Scalar(key: any, node: any) {
      // console.log('Scalar', key, node)
      if (typeof node.value === "string") {
        if (
          node.type === yaml2.Scalar.QUOTE_DOUBLE ||
          node.type === yaml2.Scalar.QUOTE_SINGLE
        ) {
          // const seq = new yaml2.YAMLSeq();
          // ['`', node.value].map(seq.add);
          // return seq;

          const quotedSeq = yaml2.parseDocument("- `\n- " + node.value).contents;

          // const col = new yaml2.YAMLSeq(['`', node.value] as any,)
          // console.log('q', quotedSeq)
          return quotedSeq as any;
          // return undefined
        }
      }

      return undefined;
      // if (
      //   key !== 'key' &&
      //   typeof node.value === 'string' &&
      //   node.type === 'PLAIN'
      // ) {
      //   node.type = 'QUOTE_SINGLE'
      // }
    },
  });
  
  console.log(doc.toJS())
  return doc.toJS()

  // console.log(yaml2.stringify(doc?.contents));

  // debugger;
  // assert(false)
  // console.log(typeof doc.contents)
  // assert(typeof doc.contents)
  // Deno.inspect(ast)

  // return parse(yamlStr, { schema });
}
