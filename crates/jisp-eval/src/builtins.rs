use indexmap::IndexMap;
use jisp_core::Span;
use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use std::str::FromStr;

use crate::ui;
use crate::value::BuiltinFn;
use crate::{Evaluator, RuntimeError, Value};

pub fn install_builtins(evaluator: &mut Evaluator) {
    evaluator.define_constructor("ok", 1);
    evaluator.define_constructor("err", 1);
    evaluator.define_constructor("some", 1);
    evaluator.define_constructor("none", 0);

    let builtins: &[(&str, BuiltinFn)] = &[
        ("+", add),
        ("-", subtract),
        ("*", multiply),
        ("/", divide),
        ("//", floor_divide),
        ("%", modulo),
        ("bigint", bigint),
        ("=", equal),
        ("<", less),
        (">", greater),
        ("<=", less_equal),
        (">=", greater_equal),
        ("math.abs", math_abs),
        ("math.min", math_min),
        ("math.max", math_max),
        ("math.pow", math_pow),
        ("math.sqrt", math_sqrt),
        ("math.floor", math_floor),
        ("math.ceil", math_ceil),
        ("math.round", math_round),
        ("str.is", str_is),
        ("str.from", str_from),
        ("str.cat", str_cat),
        ("str.len", str_len),
        ("str.join", str_join),
        ("str.split", str_split),
        ("str.trim", str_trim),
        ("str.upper", str_upper),
        ("str.lower", str_lower),
        ("str.has", str_has),
        ("str.starts", str_starts),
        ("str.ends", str_ends),
        ("str.replace", str_replace),
        ("str.slice", str_slice),
        ("list.is", list_is),
        ("list.len", list_len),
        ("list.get", list_get),
        ("list.first", list_first),
        ("list.last", list_last),
        ("list.rest", list_rest),
        ("list.slice", list_slice),
        ("list.map", list_map),
        ("list.filter", list_filter),
        ("list.fold", list_fold),
        ("list.some", list_some),
        ("list.every", list_every),
        ("list.has", list_has),
        ("list.cat", list_cat),
        ("list.prepend", list_prepend),
        ("list.append", list_append),
        ("obj.is", obj_is),
        ("obj.len", obj_len),
        ("obj.has", obj_has),
        ("obj.get", obj_get),
        ("obj.set", obj_set),
        ("obj.del", obj_del),
        ("obj.keys", obj_keys),
        ("obj.values", obj_values),
        ("obj.cat", obj_cat),
        ("ui.html", ui_html),
        ("result.try", result_try),
        ("result.map", result_map),
        ("result.map-err", result_map_err),
        ("result.recover", result_recover),
        ("io.println", io_println),
    ];

    for &(name, function) in builtins {
        evaluator.define_builtin(name, function);
    }
}

fn arity(arguments: &[Value], expected: usize, span: Span) -> Result<(), RuntimeError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(RuntimeError::at(
            span,
            format!("expected {expected} argument(s), got {}", arguments.len()),
        ))
    }
}

fn min_arity(arguments: &[Value], expected: usize, span: Span) -> Result<(), RuntimeError> {
    if arguments.len() >= expected {
        Ok(())
    } else {
        Err(RuntimeError::at(
            span,
            format!(
                "expected at least {expected} argument(s), got {}",
                arguments.len()
            ),
        ))
    }
}

fn expect_int(value: &Value, span: Span) -> Result<i64, RuntimeError> {
    match value {
        Value::Int(value) => Ok(*value),
        other => Err(RuntimeError::at(
            span,
            format!("expected int, got {}", other.type_name()),
        )),
    }
}

fn expect_bigint<'a>(value: &'a Value, span: Span) -> Result<&'a BigInt, RuntimeError> {
    match value {
        Value::BigInt(value) => Ok(value),
        other => Err(RuntimeError::at(
            span,
            format!("expected bigint, got {}", other.type_name()),
        )),
    }
}

fn expect_float(value: &Value, span: Span) -> Result<f64, RuntimeError> {
    match value {
        Value::Float(value) => Ok(*value),
        other => Err(RuntimeError::at(
            span,
            format!("expected float, got {}", other.type_name()),
        )),
    }
}

fn expect_str<'a>(value: &'a Value, span: Span) -> Result<&'a str, RuntimeError> {
    match value {
        Value::Str(value) => Ok(value),
        other => Err(RuntimeError::at(
            span,
            format!("expected str, got {}", other.type_name()),
        )),
    }
}

fn expect_list<'a>(value: &'a Value, span: Span) -> Result<&'a [Value], RuntimeError> {
    match value {
        Value::List(value) => Ok(value),
        other => Err(RuntimeError::at(
            span,
            format!("expected list, got {}", other.type_name()),
        )),
    }
}

fn expect_obj<'a>(
    value: &'a Value,
    span: Span,
) -> Result<&'a IndexMap<String, Value>, RuntimeError> {
    match value {
        Value::Obj(value) => Ok(value),
        other => Err(RuntimeError::at(
            span,
            format!("expected obj, got {}", other.type_name()),
        )),
    }
}

fn bigint(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    let value = expect_str(&args[0], span)?;
    BigInt::from_str(value)
        .map(Value::BigInt)
        .map_err(|_| RuntimeError::at(span, "invalid bigint literal"))
}

fn add(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    min_arity(args, 2, span)?;
    match &args[0] {
        Value::Int(_) => args
            .iter()
            .try_fold(0_i64, |acc, value| {
                acc.checked_add(expect_int(value, span)?)
                    .ok_or_else(|| RuntimeError::at(span, "integer overflow"))
            })
            .map(Value::Int),
        Value::BigInt(_) => args
            .iter()
            .try_fold(BigInt::zero(), |acc, value| {
                Ok(acc + expect_bigint(value, span)?)
            })
            .map(Value::BigInt),
        Value::Float(_) => args
            .iter()
            .try_fold(0.0, |acc, value| Ok(acc + expect_float(value, span)?))
            .map(Value::Float),
        other => Err(RuntimeError::at(
            span,
            format!("+ expects numbers, got {}", other.type_name()),
        )),
    }
}

fn subtract(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    min_arity(args, 2, span)?;
    match &args[0] {
        Value::Int(first) => args[1..]
            .iter()
            .try_fold(*first, |acc, value| {
                acc.checked_sub(expect_int(value, span)?)
                    .ok_or_else(|| RuntimeError::at(span, "integer overflow"))
            })
            .map(Value::Int),
        Value::BigInt(first) => args[1..]
            .iter()
            .try_fold(first.clone(), |acc, value| {
                Ok(acc - expect_bigint(value, span)?)
            })
            .map(Value::BigInt),
        Value::Float(first) => args[1..]
            .iter()
            .try_fold(*first, |acc, value| Ok(acc - expect_float(value, span)?))
            .map(Value::Float),
        other => Err(RuntimeError::at(
            span,
            format!("- expects numbers, got {}", other.type_name()),
        )),
    }
}

fn multiply(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    min_arity(args, 2, span)?;
    match &args[0] {
        Value::Int(_) => args
            .iter()
            .try_fold(1_i64, |acc, value| {
                acc.checked_mul(expect_int(value, span)?)
                    .ok_or_else(|| RuntimeError::at(span, "integer overflow"))
            })
            .map(Value::Int),
        Value::BigInt(_) => args
            .iter()
            .try_fold(BigInt::from(1), |acc, value| {
                Ok(acc * expect_bigint(value, span)?)
            })
            .map(Value::BigInt),
        Value::Float(_) => args
            .iter()
            .try_fold(1.0, |acc, value| Ok(acc * expect_float(value, span)?))
            .map(Value::Float),
        other => Err(RuntimeError::at(
            span,
            format!("* expects numbers, got {}", other.type_name()),
        )),
    }
}

fn divide(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match (&args[0], &args[1]) {
        (Value::Int(left), Value::Int(right)) => left
            .checked_div(*right)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::at(span, "division by zero or integer overflow")),
        (Value::BigInt(left), Value::BigInt(right)) if !right.is_zero() => {
            Ok(Value::BigInt(left / right))
        }
        (Value::BigInt(_), Value::BigInt(_)) => Err(RuntimeError::at(span, "division by zero")),
        (Value::Float(left), Value::Float(right)) if *right != 0.0 => {
            Ok(Value::Float(left / right))
        }
        (Value::Float(_), Value::Float(_)) => Err(RuntimeError::at(span, "division by zero")),
        _ => Err(RuntimeError::at(
            span,
            "/ requires two values of the same numeric type",
        )),
    }
}

fn floor_divide(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match (&args[0], &args[1]) {
        (Value::Int(left), Value::Int(right)) => jisp_runtime::math::floor_div_i64(*left, *right)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::at(span, "division by zero or integer overflow")),
        (Value::BigInt(left), Value::BigInt(right)) => div_euclid_bigint(left, right)
            .map(Value::BigInt)
            .ok_or_else(|| RuntimeError::at(span, "division by zero")),
        (Value::Float(left), Value::Float(right)) if *right != 0.0 => {
            Ok(Value::Float((left / right).floor()))
        }
        _ => Err(RuntimeError::at(
            span,
            "// requires two values of the same numeric type",
        )),
    }
}

fn modulo(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match (&args[0], &args[1]) {
        (Value::Int(left), Value::Int(right)) => jisp_runtime::math::modulo_i64(*left, *right)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::at(span, "modulo by zero or integer overflow")),
        (Value::BigInt(left), Value::BigInt(right)) => mod_euclid_bigint(left, right)
            .map(Value::BigInt)
            .ok_or_else(|| RuntimeError::at(span, "modulo by zero")),
        (Value::Float(left), Value::Float(right)) if *right != 0.0 => {
            Ok(Value::Float(left.rem_euclid(*right)))
        }
        _ => Err(RuntimeError::at(
            span,
            "% requires two values of the same numeric type",
        )),
    }
}

fn div_euclid_bigint(left: &BigInt, right: &BigInt) -> Option<BigInt> {
    if right.is_zero() {
        return None;
    }
    let quotient = left / right;
    let remainder = left % right;
    if remainder.is_negative() {
        if right.is_positive() {
            Some(quotient - 1)
        } else {
            Some(quotient + 1)
        }
    } else {
        Some(quotient)
    }
}

fn mod_euclid_bigint(left: &BigInt, right: &BigInt) -> Option<BigInt> {
    if right.is_zero() {
        return None;
    }
    let remainder = left % right;
    if remainder.is_negative() {
        if right.is_positive() {
            Some(remainder + right)
        } else {
            Some(remainder - right)
        }
    } else {
        Some(remainder)
    }
}

fn equal(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    Ok(Value::Bool(args[0].structurally_equal(&args[1])?))
}

fn compare(
    args: &[Value],
    span: Span,
    int: impl FnOnce(i64, i64) -> bool,
    bigint: impl FnOnce(&BigInt, &BigInt) -> bool,
    float: impl FnOnce(f64, f64) -> bool,
    string: impl FnOnce(&str, &str) -> bool,
) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(int(*a, *b))),
        (Value::BigInt(a), Value::BigInt(b)) => Ok(Value::Bool(bigint(a, b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(float(*a, *b))),
        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(string(a, b))),
        _ => Err(RuntimeError::at(
            span,
            "comparison requires values of the same ordered type",
        )),
    }
}

fn less(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    compare(
        args,
        span,
        |a, b| a < b,
        |a, b| a < b,
        |a, b| a < b,
        |a, b| a < b,
    )
}
fn greater(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    compare(
        args,
        span,
        |a, b| a > b,
        |a, b| a > b,
        |a, b| a > b,
        |a, b| a > b,
    )
}
fn less_equal(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    compare(
        args,
        span,
        |a, b| a <= b,
        |a, b| a <= b,
        |a, b| a <= b,
        |a, b| a <= b,
    )
}
fn greater_equal(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    compare(
        args,
        span,
        |a, b| a >= b,
        |a, b| a >= b,
        |a, b| a >= b,
        |a, b| a >= b,
    )
}

fn math_abs(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    match args[0] {
        Value::Int(value) => value
            .checked_abs()
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::at(span, "integer overflow")),
        Value::BigInt(ref value) => Ok(Value::BigInt(value.abs())),
        Value::Float(value) => Ok(Value::Float(value.abs())),
        ref other => Err(RuntimeError::at(
            span,
            format!("math.abs expects a number, got {}", other.type_name()),
        )),
    }
}

fn math_min(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int((*a).min(*b))),
        (Value::BigInt(a), Value::BigInt(b)) => Ok(Value::BigInt(a.min(b).clone())),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
        _ => Err(RuntimeError::at(
            span,
            "math.min requires matching numeric types",
        )),
    }
}

fn math_max(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int((*a).max(*b))),
        (Value::BigInt(a), Value::BigInt(b)) => Ok(Value::BigInt(a.max(b).clone())),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
        _ => Err(RuntimeError::at(
            span,
            "math.max requires matching numeric types",
        )),
    }
}

fn math_pow(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) if *b >= 0 => a
            .checked_pow(*b as u32)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::at(span, "integer overflow")),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(*b))),
        _ => Err(RuntimeError::at(
            span,
            "math.pow requires matching numeric types",
        )),
    }
}

fn math_sqrt(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    let value = expect_float(&args[0], span)?;
    Ok(Value::Float(value.sqrt()))
}
fn math_floor(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Float(expect_float(&args[0], span)?.floor()))
}
fn math_ceil(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Float(expect_float(&args[0], span)?.ceil()))
}
fn math_round(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Float(expect_float(&args[0], span)?.round()))
}

fn str_is(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Bool(matches!(args[0], Value::Str(_))))
}
fn str_from(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::string(args[0].display_string()))
}
fn str_cat(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    let parts = args
        .iter()
        .map(|value| expect_str(value, span))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::string(jisp_runtime::string::cat(parts)))
}
fn str_len(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Int(
        expect_str(&args[0], span)?.chars().count() as i64
    ))
}
fn str_join(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let delimiter = expect_str(&args[0], span)?;
    let list = expect_list(&args[1], span)?;
    let strings = list
        .iter()
        .map(|value| expect_str(value, span))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::string(strings.join(delimiter)))
}
fn str_split(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let value = expect_str(&args[0], span)?;
    let delimiter = expect_str(&args[1], span)?;
    Ok(Value::List(
        value
            .split(delimiter)
            .map(|part| Value::string(part.to_owned()))
            .collect(),
    ))
}
fn str_trim(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::string(expect_str(&args[0], span)?.trim().to_owned()))
}
fn str_upper(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::string(expect_str(&args[0], span)?.to_uppercase()))
}
fn str_lower(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::string(expect_str(&args[0], span)?.to_lowercase()))
}
fn str_has(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    Ok(Value::Bool(
        expect_str(&args[0], span)?.contains(expect_str(&args[1], span)?),
    ))
}
fn str_starts(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    Ok(Value::Bool(
        expect_str(&args[0], span)?.starts_with(expect_str(&args[1], span)?),
    ))
}
fn str_ends(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    Ok(Value::Bool(
        expect_str(&args[0], span)?.ends_with(expect_str(&args[1], span)?),
    ))
}
fn str_replace(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 3, span)?;
    Ok(Value::string(expect_str(&args[0], span)?.replace(
        expect_str(&args[1], span)?,
        expect_str(&args[2], span)?,
    )))
}
fn str_slice(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 3, span)?;
    let value = expect_str(&args[0], span)?;
    let start = expect_int(&args[1], span)?;
    let end = expect_int(&args[2], span)?;
    if start < 0 || end < 0 {
        return Ok(err("string slice indices cannot be negative"));
    }
    Ok(
        match jisp_runtime::string::slice(value, start as usize, end as usize) {
            Some(value) => ok(Value::string(value)),
            None => err("string slice is out of bounds"),
        },
    )
}

fn list_is(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Bool(matches!(args[0], Value::List(_))))
}
fn list_len(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Int(expect_list(&args[0], span)?.len() as i64))
}
fn list_get(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let list = expect_list(&args[0], span)?;
    let index = expect_int(&args[1], span)?;
    if index < 0 {
        return Ok(err("list index cannot be negative"));
    }
    Ok(list
        .get(index as usize)
        .cloned()
        .map(ok)
        .unwrap_or_else(|| err("list index is out of bounds")))
}
fn list_first(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(expect_list(&args[0], span)?
        .first()
        .cloned()
        .map(ok)
        .unwrap_or_else(|| err("list is empty")))
}
fn list_last(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(expect_list(&args[0], span)?
        .last()
        .cloned()
        .map(ok)
        .unwrap_or_else(|| err("list is empty")))
}
fn list_rest(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    let list = expect_list(&args[0], span)?;
    Ok(Value::List(list.get(1..).unwrap_or_default().to_vec()))
}
fn list_slice(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 3, span)?;
    let list = expect_list(&args[0], span)?;
    let start = expect_int(&args[1], span)?;
    let end = expect_int(&args[2], span)?;
    if start < 0 || end < 0 {
        return Ok(err("list slice indices cannot be negative"));
    }
    Ok(
        match jisp_runtime::list::slice(list, start as usize, end as usize) {
            Some(value) => ok(Value::List(value)),
            None => err("list slice is out of bounds"),
        },
    )
}
fn list_map(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let function = args[0].clone();
    let list = expect_list(&args[1], span)?;
    let mut output = Vec::with_capacity(list.len());
    for value in list {
        output.push(eval.apply(function.clone(), std::slice::from_ref(value), span)?);
    }
    Ok(Value::List(output))
}
fn list_filter(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let function = args[0].clone();
    let list = expect_list(&args[1], span)?;
    let mut output = vec![];
    for value in list {
        if eval
            .apply(function.clone(), std::slice::from_ref(value), span)?
            .truthy()
        {
            output.push(value.clone());
        }
    }
    Ok(Value::List(output))
}
fn list_fold(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 3, span)?;
    let function = args[0].clone();
    let mut accumulator = args[1].clone();
    for value in expect_list(&args[2], span)? {
        accumulator = eval.apply(function.clone(), &[accumulator, value.clone()], span)?;
    }
    Ok(accumulator)
}
fn list_some(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    for value in expect_list(&args[1], span)? {
        if eval
            .apply(args[0].clone(), std::slice::from_ref(value), span)?
            .truthy()
        {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}
fn list_every(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    for value in expect_list(&args[1], span)? {
        if !eval
            .apply(args[0].clone(), std::slice::from_ref(value), span)?
            .truthy()
        {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}
fn list_has(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    for value in expect_list(&args[0], span)? {
        if value.structurally_equal(&args[1])? {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}
fn list_cat(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    let mut output = vec![];
    for value in args {
        output.extend(expect_list(value, span)?.iter().cloned());
    }
    Ok(Value::List(output))
}
fn list_prepend(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let list = expect_list(&args[1], span)?.to_vec();
    Ok(Value::List(jisp_runtime::list::prepend(
        args[0].clone(),
        list,
    )))
}
fn list_append(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let list = expect_list(&args[0], span)?.to_vec();
    Ok(Value::List(jisp_runtime::list::append(
        list,
        args[1].clone(),
    )))
}

fn obj_is(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Bool(matches!(args[0], Value::Obj(_))))
}
fn obj_len(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::Int(expect_obj(&args[0], span)?.len() as i64))
}
fn obj_has(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    Ok(Value::Bool(
        expect_obj(&args[0], span)?.contains_key(expect_str(&args[1], span)?),
    ))
}
fn obj_get(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let object = expect_obj(&args[0], span)?;
    let key = expect_str(&args[1], span)?;
    Ok(object
        .get(key)
        .cloned()
        .map(ok)
        .unwrap_or_else(|| err(format!("object has no key `{key}`"))))
}
fn obj_set(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 3, span)?;
    let object = expect_obj(&args[0], span)?;
    let key = expect_str(&args[1], span)?.to_owned();
    Ok(Value::Obj(jisp_runtime::object::set(
        object,
        key,
        args[2].clone(),
    )))
}
fn obj_del(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    let object = expect_obj(&args[0], span)?;
    let key = expect_str(&args[1], span)?;
    Ok(Value::Obj(jisp_runtime::object::delete(object, key)))
}
fn obj_keys(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::List(
        expect_obj(&args[0], span)?
            .keys()
            .cloned()
            .map(Value::string)
            .collect(),
    ))
}
fn obj_values(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::List(
        expect_obj(&args[0], span)?.values().cloned().collect(),
    ))
}
fn obj_cat(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    let objects = args
        .iter()
        .map(|value| expect_obj(value, span).cloned())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::Obj(jisp_runtime::object::concat(objects)))
}

fn ui_html(_: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 1, span)?;
    Ok(Value::string(ui::render_html(&args[0], span)?))
}

fn result_try(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match &args[0] {
        Value::Variant { tag, fields } if tag == "ok" && fields.len() == 1 => {
            eval.apply(args[1].clone(), &fields[..], span)
        }
        Value::Variant { tag, fields } if tag == "err" && fields.len() == 1 => Ok(args[0].clone()),
        _ => Err(RuntimeError::at(
            span,
            "result.try expects [ok, value] or [err, error]",
        )),
    }
}
fn result_map(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match &args[0] {
        Value::Variant { tag, fields } if tag == "ok" && fields.len() == 1 => {
            Ok(ok(eval.apply(args[1].clone(), &fields[..], span)?))
        }
        Value::Variant { tag, fields } if tag == "err" && fields.len() == 1 => Ok(args[0].clone()),
        _ => Err(RuntimeError::at(span, "result.map expects a Result")),
    }
}
fn result_map_err(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match &args[0] {
        Value::Variant { tag, fields } if tag == "err" && fields.len() == 1 => {
            Ok(err(eval.apply(args[1].clone(), &fields[..], span)?))
        }
        Value::Variant { tag, fields } if tag == "ok" && fields.len() == 1 => Ok(args[0].clone()),
        _ => Err(RuntimeError::at(span, "result.map-err expects a Result")),
    }
}
fn result_recover(eval: &mut Evaluator, args: &[Value], span: Span) -> Result<Value, RuntimeError> {
    arity(args, 2, span)?;
    match &args[0] {
        Value::Variant { tag, fields } if tag == "err" && fields.len() == 1 => {
            eval.apply(args[1].clone(), &fields[..], span)
        }
        Value::Variant { tag, fields } if tag == "ok" && fields.len() == 1 => Ok(args[0].clone()),
        _ => Err(RuntimeError::at(span, "result.recover expects a Result")),
    }
}

fn io_println(_: &mut Evaluator, args: &[Value], _: Span) -> Result<Value, RuntimeError> {
    println!(
        "{}",
        args.iter()
            .map(Value::display_string)
            .collect::<Vec<_>>()
            .join(" ")
    );
    Ok(Value::Null)
}

fn ok(value: Value) -> Value {
    Value::Variant {
        tag: "ok".to_owned(),
        fields: vec![value],
    }
}

fn err(value: impl IntoErrorValue) -> Value {
    Value::Variant {
        tag: "err".to_owned(),
        fields: vec![value.into_error_value()],
    }
}

trait IntoErrorValue {
    fn into_error_value(self) -> Value;
}

impl IntoErrorValue for Value {
    fn into_error_value(self) -> Value {
        self
    }
}

impl IntoErrorValue for &str {
    fn into_error_value(self) -> Value {
        Value::string(self.to_owned())
    }
}

impl IntoErrorValue for String {
    fn into_error_value(self) -> Value {
        Value::string(self)
    }
}
