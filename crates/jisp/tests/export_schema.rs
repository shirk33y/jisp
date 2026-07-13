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
