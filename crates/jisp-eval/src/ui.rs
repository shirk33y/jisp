use jisp_core::Span;

use crate::{RuntimeError, Value};

pub(crate) fn render_html(value: &Value, span: Span) -> Result<String, RuntimeError> {
    let mut output = String::new();
    render_node(value, span, &mut output)?;
    Ok(output)
}

fn render_node(value: &Value, span: Span, output: &mut String) -> Result<(), RuntimeError> {
    let Value::Obj(node) = value else {
        return Err(RuntimeError::at(
            span,
            format!("ui node must be an obj, got {}", value.type_name()),
        ));
    };
    let tag = expect_string_field(node.get("tag"), "tag", span)?;
    if tag == "text" {
        let text = expect_string_field(node.get("value"), "value", span)?;
        escape_text(text, output);
        return Ok(());
    }
    validate_tag(tag, span)?;

    output.push('<');
    output.push_str(tag);
    render_classes(node.get("classes"), span, output)?;
    for (key, value) in node {
        if matches!(
            key.as_str(),
            "tag" | "attrs" | "props" | "classes" | "children" | "events" | "key" | "value"
        ) {
            continue;
        }
        render_attribute(key, value, span, output)?;
    }
    render_attribute_object(node.get("attrs"), "attrs", span, output)?;
    render_attribute_object(node.get("props"), "props", span, output)?;
    output.push('>');

    if let Some(children) = node.get("children") {
        let Value::List(children) = children else {
            return Err(RuntimeError::at(span, "ui children must be a list"));
        };
        render_children(children, span, output)?;
    }

    output.push_str("</");
    output.push_str(tag);
    output.push('>');
    Ok(())
}

fn render_children(
    children: &[Value],
    span: Span,
    output: &mut String,
) -> Result<(), RuntimeError> {
    for child in children {
        match child {
            Value::Null => {}
            Value::List(children) => render_children(children, span, output)?,
            child => render_node(child, span, output)?,
        }
    }
    Ok(())
}

fn render_classes(
    value: Option<&Value>,
    span: Span,
    output: &mut String,
) -> Result<(), RuntimeError> {
    let Some(value) = value else {
        return Ok(());
    };
    let Value::Obj(classes) = value else {
        return Err(RuntimeError::at(span, "ui classes must be an obj"));
    };

    let mut active = vec![];
    for (class, enabled) in classes {
        match enabled {
            Value::Bool(true) => active.push(class.as_str()),
            Value::Bool(false) => {}
            other => {
                return Err(RuntimeError::at(
                    span,
                    format!("ui class flags must be bool, got {}", other.type_name()),
                ));
            }
        }
    }

    if active.is_empty() {
        return Ok(());
    }
    output.push_str(" class=\"");
    for (index, class) in active.iter().enumerate() {
        if index > 0 {
            output.push(' ');
        }
        escape_attribute(class, output);
    }
    output.push('"');
    Ok(())
}

fn render_attribute(
    key: &str,
    value: &Value,
    span: Span,
    output: &mut String,
) -> Result<(), RuntimeError> {
    validate_attribute(key, span)?;
    match value {
        Value::Null | Value::Bool(false) => Ok(()),
        Value::Bool(true) => {
            output.push(' ');
            output.push_str(key);
            Ok(())
        }
        Value::Str(value) => {
            validate_attribute_value(key, value, span)?;
            output.push(' ');
            output.push_str(key);
            output.push_str("=\"");
            escape_attribute(value, output);
            output.push('"');
            Ok(())
        }
        Value::Int(_) | Value::BigInt(_) | Value::Float(_) => {
            output.push(' ');
            output.push_str(key);
            output.push_str("=\"");
            escape_attribute(&value.display_string(), output);
            output.push('"');
            Ok(())
        }
        other => Err(RuntimeError::at(
            span,
            format!(
                "ui attribute values must be scalar, got {}",
                other.type_name()
            ),
        )),
    }
}

fn render_attribute_object(
    value: Option<&Value>,
    field: &str,
    span: Span,
    output: &mut String,
) -> Result<(), RuntimeError> {
    let Some(value) = value else {
        return Ok(());
    };
    let Value::Obj(attributes) = value else {
        return Err(RuntimeError::at(span, format!("ui {field} must be an obj")));
    };
    for (key, value) in attributes {
        render_attribute(key, value, span, output)?;
    }
    Ok(())
}

fn expect_string_field<'a>(
    value: Option<&'a Value>,
    field: &str,
    span: Span,
) -> Result<&'a str, RuntimeError> {
    match value {
        Some(Value::Str(value)) => Ok(value),
        Some(other) => Err(RuntimeError::at(
            span,
            format!("ui field `{field}` must be str, got {}", other.type_name()),
        )),
        None => Err(RuntimeError::at(
            span,
            format!("ui node is missing `{field}`"),
        )),
    }
}

fn validate_tag(tag: &str, span: Span) -> Result<(), RuntimeError> {
    if is_html_name(tag) {
        Ok(())
    } else {
        Err(RuntimeError::at(span, format!("invalid ui tag `{tag}`")))
    }
}

fn validate_attribute(attribute: &str, span: Span) -> Result<(), RuntimeError> {
    if !is_html_name(attribute) {
        Err(RuntimeError::at(
            span,
            format!("invalid ui attribute `{attribute}`"),
        ))
    } else if attribute.to_ascii_lowercase().starts_with("on") {
        Err(RuntimeError::at(
            span,
            format!(
                "ui event attribute `{attribute}` is not allowed; use the `on` directive instead"
            ),
        ))
    } else {
        Ok(())
    }
}

fn validate_attribute_value(attribute: &str, value: &str, span: Span) -> Result<(), RuntimeError> {
    if matches!(attribute.to_ascii_lowercase().as_str(), "href" | "src")
        && value
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("javascript:")
    {
        return Err(RuntimeError::at(
            span,
            format!("ui {attribute} must not use a javascript: URL"),
        ));
    }
    Ok(())
}

fn is_html_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

fn escape_text(value: &str, output: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(ch),
        }
    }
}

fn escape_attribute(value: &str, output: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
}
