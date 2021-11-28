import * as YAML from "yaml2";

export function toMinimal(yamlStr: string) {
  const doc = YAML.parseDocument(yamlStr);

  YAML.visit(doc, {
    Scalar(_key: any, node: any) {
      // quote string
      //
      // plain     -> symbol  -> "plain"
      // "quoted"  -> string  -> ["`", "quoted"]
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

// export function to
