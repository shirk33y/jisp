use std::fs;
use std::path::PathBuf;

#[test]
fn export_schema_describes_closed_json_native_values() {
    let schema = jisp::export_schema(
        "config.lisp",
        r#"
(export config
  (obj "name" "Ada" "enabled" true "retries" (list 1 2)))
"#,
        "config",
    )
    .unwrap();

    let value = &schema["schema"];
    assert_eq!(value["type"], "object");
    assert_eq!(value["properties"]["name"]["type"], "string");
    assert_eq!(value["properties"]["enabled"]["type"], "boolean");
    assert_eq!(value["properties"]["retries"]["items"]["type"], "integer");
    assert_eq!(value["additionalProperties"], false);
}

#[test]
fn export_schema_rejects_functions() {
    let error =
        jisp::export_schema("config.lisp", "(export config (fn () 1))", "config").unwrap_err();

    assert!(error
        .to_string()
        .contains("functions have no JSON representation"));
}

#[test]
fn export_schema_describes_named_variants_as_tagged_arrays() {
    let schema = jisp::export_schema(
        "response.lisp",
        r#"
(type response
  (ok int)
  (err str))
(export value (ok 42))
"#,
        "value",
    )
    .unwrap();

    assert_eq!(schema["schema"]["$ref"], "#/$defs/response");
    let variants = schema["$defs"]["response"]["oneOf"].as_array().unwrap();
    assert_eq!(variants[0]["prefixItems"][0]["const"], "ok");
    assert_eq!(variants[0]["prefixItems"][1]["type"], "integer");
    assert_eq!(variants[1]["prefixItems"][0]["const"], "err");
    assert_eq!(variants[1]["prefixItems"][1]["type"], "string");
}

#[test]
fn export_schema_instantiates_polymorphic_exports() {
    let error = jisp::export_schema("values.lisp", "(export values (list))", "values").unwrap_err();
    assert!(error
        .to_string()
        .contains("needs an explicit instantiation"));

    let schema = jisp::export_schema_with_type(
        "values.lisp",
        "(export values (list))",
        "values",
        Some("(list int)"),
    )
    .unwrap();

    assert_eq!(schema["schema"]["type"], "array");
    assert_eq!(schema["schema"]["items"]["type"], "integer");
}

#[test]
fn export_schema_instantiates_generic_named_variants() {
    let schema = jisp::export_schema_with_type(
        "box.lisp",
        r#"
(type box
  (boxed a))
(export value (boxed 42))
"#,
        "value",
        Some("(box int)"),
    )
    .unwrap();

    assert_eq!(schema["schema"]["$ref"], "#/$defs/box_int");
    let variants = schema["$defs"]["box_int"]["oneOf"].as_array().unwrap();
    assert_eq!(variants[0]["prefixItems"][0]["const"], "boxed");
    assert_eq!(variants[0]["prefixItems"][1]["type"], "integer");
}

#[test]
fn export_schema_describes_recursive_named_variants() {
    let schema = jisp::export_schema(
        "tree.lisp",
        r#"
(type tree
  (leaf int)
  (node tree tree))
(export value (leaf 1))
"#,
        "value",
    )
    .unwrap();

    assert_eq!(schema["schema"]["$ref"], "#/$defs/tree");
    let variants = schema["$defs"]["tree"]["oneOf"].as_array().unwrap();
    assert_eq!(variants[1]["prefixItems"][0]["const"], "node");
    assert_eq!(variants[1]["prefixItems"][1]["$ref"], "#/$defs/tree");
    assert_eq!(variants[1]["prefixItems"][2]["$ref"], "#/$defs/tree");
}

#[test]
fn export_schema_describes_recursive_generic_variants() {
    let schema = jisp::export_schema_with_type(
        "tree.lisp",
        r#"
(type tree
  (leaf a)
  (node (tree a) (tree a)))
(export value (leaf 1))
"#,
        "value",
        Some("(tree int)"),
    )
    .unwrap();

    assert_eq!(schema["schema"]["$ref"], "#/$defs/tree_int");
    let variants = schema["$defs"]["tree_int"]["oneOf"].as_array().unwrap();
    assert_eq!(variants[0]["prefixItems"][1]["type"], "integer");
    assert_eq!(variants[1]["prefixItems"][1]["$ref"], "#/$defs/tree_int");
    assert_eq!(variants[1]["prefixItems"][2]["$ref"], "#/$defs/tree_int");
}

#[test]
fn export_schema_describes_imported_recursive_generic_variants() {
    let dir = fixture_dir("imported-recursive-schema");
    let tree = dir.join("tree.lisp");
    let main = dir.join("main.lisp");
    fs::write(
        &tree,
        r#"
(type tree
  (leaf a)
  (node (tree a) (tree a)))
(export sample (leaf 1))
"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import tree "tree.lisp")
(export value tree.sample)
"#,
    )
    .unwrap();
    let text = fs::read_to_string(&main).unwrap();

    let schema = jisp::export_schema_with_type(&main, &text, "value", Some("(tree int)")).unwrap();

    assert_eq!(schema["schema"]["$ref"], "#/$defs/tree_int");
    let variants = schema["$defs"]["tree_int"]["oneOf"].as_array().unwrap();
    assert_eq!(variants[0]["prefixItems"][1]["type"], "integer");
    assert_eq!(variants[1]["prefixItems"][1]["$ref"], "#/$defs/tree_int");
}

fn fixture_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/jisp-schema-fixtures")
        .join(format!("{}-{}", name, std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}
