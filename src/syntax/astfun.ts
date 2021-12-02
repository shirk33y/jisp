import * as YAML from "yaml2";

export function toMinimal(yamlStr: string) {
  const doc = YAML.parseDocument(yamlStr);

  YAML.visit(doc, {
    Scalar(_key, node: any) {
      /**
       * Quote "str" (yaml) => ["`", "str"] (json)
       *
       * plain     -> symbol  -> "plain"
       * "str"  -> string  -> ["`", "str"]
       */
      if (typeof node.value === "string") {
        if (
          node.type === YAML.Scalar.QUOTE_DOUBLE ||
          node.type === YAML.Scalar.QUOTE_SINGLE
        ) {
          return YAML.parseDocument(`- str\n- ${node.value}`).contents as any;
        }
      }
    },
  });

  return doc.toJS();
}

export function fromMinimal(jsonStr: string) {
  const doc = YAML.parseDocument(jsonStr);

  /*
   * Convert ["`", "str"] (json) => "str" (yaml)
   *
   * 1. temporary replace ["`", "str"] with 'str' (single quote)
   * 2. replace "foo" with foo
   * 3. replace 'str' with "str"
   * 
   */
  YAML.visit(doc, {
    Seq(_key, node: YAML.YAMLSeq) {
      if (
        node.items.length === 2 &&
        node.items.every(YAML.isScalar) &&
        node.items[0].value === "`" &&
        node.items[1].type === YAML.Scalar.QUOTE_DOUBLE
      ) {
        const rep = node.items[1].clone() as YAML.Scalar;
        rep.type = YAML.Scalar.QUOTE_SINGLE;

        return rep;
      }
    },
  });

  YAML.visit(doc, {
    Scalar(_key, node: YAML.Scalar) {
      if (node.type === YAML.Scalar.QUOTE_DOUBLE) {
        node.type = YAML.Scalar.PLAIN;
      } else if (node.type === YAML.Scalar.QUOTE_SINGLE) {
        node.type = YAML.Scalar.QUOTE_DOUBLE;
      }
    },
  });

  return YAML.stringify(doc);
}
