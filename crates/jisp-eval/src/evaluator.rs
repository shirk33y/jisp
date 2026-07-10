use std::collections::HashMap;

use indexmap::IndexMap;
use jisp_core::Span;
use jisp_ir::{
    CaseBranch, Expr, ExprKind, Literal, Module, Pattern, StringPart,
};

use crate::builtins::install_builtins;
use crate::{Builtin, Closure, Constructor, Env, RuntimeError, Value};

#[derive(Clone)]
pub struct LoadedModule {
    pub env: Env,
    pub exports: HashMap<String, Value>,
}

pub struct Evaluator {
    root: Env,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    pub fn new() -> Self {
        let root = Env::root();
        let mut evaluator = Self { root };
        install_builtins(&mut evaluator);
        evaluator
    }

    pub fn root_env(&self) -> Env {
        self.root.clone()
    }

    pub fn define_builtin(&mut self, name: &'static str, function: crate::value::BuiltinFn) {
        self.root
            .define(name, Value::Builtin(Builtin { name, function }));
    }

    pub fn define_constructor(&mut self, name: impl Into<String>, arity: usize) {
        let name = name.into();
        self.root.define(
            name.clone(),
            Value::Constructor(Constructor { name, arity }),
        );
    }

    pub fn load_module(&mut self, module: &Module) -> Result<LoadedModule, RuntimeError> {
        if let Some(import) = module.imports.first() {
            return Err(RuntimeError::at(
                import.span,
                "module loading is not implemented; import resolution belongs in jisp-core",
            ));
        }

        let env = self.root.child();

        for declaration in &module.types {
            for variant in &declaration.variants {
                env.define(
                    variant.name.clone(),
                    Value::Constructor(Constructor {
                        name: variant.name.clone(),
                        arity: variant.field_types.len(),
                    }),
                );
            }
        }

        for definition in &module.definitions {
            env.define_placeholder(definition.name.clone());
        }

        for definition in &module.definitions {
            let value = self.eval_in(&definition.value, &env)?;
            env.assign(&definition.name, value)?;
        }

        let mut exports = HashMap::new();
        for name in &module.exports {
            exports.insert(name.clone(), env.lookup(name)?);
        }

        Ok(LoadedModule { env, exports })
    }

    pub fn run_main(&mut self, module: &Module) -> Result<Value, RuntimeError> {
        let loaded = self.load_module(module)?;
        let main = loaded.env.lookup("main")?;
        self.apply(main, &[], module.definitions.first().map(|d| d.span).unwrap_or_else(|| {
            Span::empty(jisp_core::SourceId(0), 0)
        }))
    }

    pub fn eval_in(&mut self, expr: &Expr, env: &Env) -> Result<Value, RuntimeError> {
        let result = match &expr.kind {
            ExprKind::Literal(literal) => Ok(literal_value(literal)),
            ExprKind::Name(name) => env.lookup(name).map_err(|error| RuntimeError {
                span: Some(expr.span),
                ..error
            }),
            ExprKind::Lambda { params, rest, body } => Ok(Value::Closure(Closure {
                params: params.clone(),
                rest: rest.clone(),
                body: (**body).clone(),
                env: env.clone(),
            })),
            ExprKind::Let { bindings, body } => {
                let scope = env.child();
                for (name, value) in bindings {
                    let value = self.eval_in(value, &scope)?;
                    scope.define(name.clone(), value);
                }
                self.eval_in(body, &scope)
            }
            ExprKind::Do(expressions) => {
                let mut value = Value::Null;
                for expression in expressions {
                    value = self.eval_in(expression, env)?;
                }
                Ok(value)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.eval_in(condition, env)?.truthy() {
                    self.eval_in(then_branch, env)
                } else {
                    self.eval_in(else_branch, env)
                }
            }
            ExprKind::And(expressions) => {
                let mut value = Value::Bool(true);
                for expression in expressions {
                    value = self.eval_in(expression, env)?;
                    if !value.truthy() {
                        break;
                    }
                }
                Ok(value)
            }
            ExprKind::Or(expressions) => {
                let mut value = Value::Null;
                for expression in expressions {
                    value = self.eval_in(expression, env)?;
                    if value.truthy() {
                        break;
                    }
                }
                Ok(value)
            }
            ExprKind::Not(expression) => Ok(Value::Bool(!self.eval_in(expression, env)?.truthy())),
            ExprKind::Call { callee, arguments } => {
                let callee = self.eval_in(callee, env)?;
                let arguments = arguments
                    .iter()
                    .map(|argument| self.eval_in(argument, env))
                    .collect::<Result<Vec<_>, _>>()?;
                self.apply(callee, &arguments, expr.span)
            }
            ExprKind::List(expressions) => Ok(Value::List(
                expressions
                    .iter()
                    .map(|expression| self.eval_in(expression, env))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            ExprKind::Object(fields) => {
                let mut object = IndexMap::new();
                for (key, value) in fields {
                    let key = expect_string(self.eval_in(key, env)?, key.span)?;
                    if object.contains_key(&key) {
                        return Err(RuntimeError::at(
                            key_span(key.as_str(), key.len(), expr.span),
                            format!("duplicate object key `{key}`"),
                        ));
                    }
                    object.insert(key, self.eval_in(value, env)?);
                }
                Ok(Value::Obj(object))
            }
            ExprKind::Field { object, key } => {
                let object = self.eval_in(object, env)?;
                let key = expect_string(self.eval_in(key, env)?, key.span)?;
                match object {
                    Value::Obj(object) => object.get(&key).cloned().ok_or_else(|| {
                        RuntimeError::at(expr.span, format!("object has no key `{key}`"))
                    }),
                    other => Err(RuntimeError::at(
                        expr.span,
                        format!("`.` expects obj, got {}", other.type_name()),
                    )),
                }
            }
            ExprKind::StringTemplate { lines, parts } => {
                self.eval_string_template(*lines, parts, env, expr.span)
            }
            ExprKind::Case { subject, branches } => {
                let value = self.eval_in(subject, env)?;
                self.eval_case(&value, branches, env, expr.span)
            }
        };

        result.map_err(|error| error.push_frame(expr.span))
    }

    pub fn apply(
        &mut self,
        callee: Value,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match callee {
            Value::Builtin(builtin) => (builtin.function)(self, arguments, span),
            Value::Constructor(constructor) => {
                if arguments.len() != constructor.arity {
                    return Err(RuntimeError::at(
                        span,
                        format!(
                            "{} expects {} argument(s), got {}",
                            constructor.name,
                            constructor.arity,
                            arguments.len()
                        ),
                    ));
                }
                Ok(Value::Variant {
                    tag: constructor.name,
                    fields: arguments.to_vec(),
                })
            }
            Value::Closure(closure) => {
                if arguments.len() < closure.params.len()
                    || (closure.rest.is_none() && arguments.len() != closure.params.len())
                {
                    return Err(RuntimeError::at(
                        span,
                        format!(
                            "function expects {}{} argument(s), got {}",
                            closure.params.len(),
                            if closure.rest.is_some() { "+" } else { "" },
                            arguments.len()
                        ),
                    ));
                }
                let env = closure.env.child();
                for (name, value) in closure.params.iter().zip(arguments) {
                    env.define(name.clone(), value.clone());
                }
                if let Some(rest) = closure.rest {
                    env.define(rest, Value::List(arguments[closure.params.len()..].to_vec()));
                }
                self.eval_in(&closure.body, &env)
            }
            other => Err(RuntimeError::at(
                span,
                format!("{} is not callable", other.type_name()),
            )),
        }
    }

    pub fn truthy(&self, value: &Value) -> bool {
        value.truthy()
    }

    fn eval_string_template(
        &mut self,
        lines: bool,
        parts: &[StringPart],
        env: &Env,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let mut fragments = vec![];
        for part in parts {
            match part {
                StringPart::Literal(value) => fragments.push(value.clone()),
                StringPart::Expr(expression) => {
                    fragments.push(expect_string(self.eval_in(expression, env)?, expression.span)?)
                }
                StringPart::Splice(expression) => {
                    let value = self.eval_in(expression, env)?;
                    let Value::List(values) = value else {
                        return Err(RuntimeError::at(
                            expression.span,
                            "string splicing expects a list of strings",
                        ));
                    };
                    for value in values {
                        fragments.push(expect_string(value, expression.span)?);
                    }
                }
            }
        }
        let value = if lines {
            jisp_runtime::string::lines(&fragments)
        } else {
            jisp_runtime::string::cat(&fragments)
        };
        Ok(Value::string(value))
    }

    fn eval_case(
        &mut self,
        value: &Value,
        branches: &[CaseBranch],
        env: &Env,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        for branch in branches {
            let mut bindings = vec![];
            if pattern_matches(&branch.pattern, value, &mut bindings)? {
                let scope = env.child();
                for (name, value) in bindings {
                    scope.define(name, value);
                }
                return self.eval_in(&branch.body, &scope);
            }
        }
        Err(RuntimeError::at(
            span,
            "non-exhaustive case reached at runtime; the typechecker should reject this",
        ))
    }
}

fn literal_value(literal: &Literal) -> Value {
    match literal {
        Literal::Null => Value::Null,
        Literal::Bool(value) => Value::Bool(*value),
        Literal::Int(value) => Value::Int(*value),
        Literal::Float(value) => Value::Float(*value),
        Literal::String(value) => Value::string(value.clone()),
    }
}

fn expect_string(value: Value, span: Span) -> Result<String, RuntimeError> {
    match value {
        Value::Str(value) => Ok(value.to_string()),
        other => Err(RuntimeError::at(
            span,
            format!("expected str, got {}", other.type_name()),
        )),
    }
}

fn pattern_matches(
    pattern: &Pattern,
    value: &Value,
    bindings: &mut Vec<(String, Value)>,
) -> Result<bool, RuntimeError> {
    match pattern {
        Pattern::Wildcard => Ok(true),
        Pattern::Bind(name) => {
            if bindings.iter().any(|(existing, _)| existing == name) {
                return Err(RuntimeError::message(format!(
                    "pattern binds `{name}` more than once"
                )));
            }
            bindings.push((name.clone(), value.clone()));
            Ok(true)
        }
        Pattern::Literal(literal) => literal_value(literal).structurally_equal(value),
        Pattern::Variant { tag, fields } => {
            let Value::Variant {
                tag: value_tag,
                fields: value_fields,
            } = value
            else {
                return Ok(false);
            };
            if tag != value_tag || fields.len() != value_fields.len() {
                return Ok(false);
            }
            for (pattern, value) in fields.iter().zip(value_fields) {
                if !pattern_matches(pattern, value, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Pattern::List { prefix, rest } => {
            let Value::List(values) = value else {
                return Ok(false);
            };
            if values.len() < prefix.len() || (rest.is_none() && values.len() != prefix.len()) {
                return Ok(false);
            }
            for (pattern, value) in prefix.iter().zip(values) {
                if !pattern_matches(pattern, value, bindings)? {
                    return Ok(false);
                }
            }
            if let Some(rest) = rest {
                bindings.push((rest.clone(), Value::List(values[prefix.len()..].to_vec())));
            }
            Ok(true)
        }
        Pattern::Object(fields) => {
            let Value::Obj(object) = value else {
                return Ok(false);
            };
            for (key, pattern) in fields {
                let Some(value) = object.get(key) else {
                    return Ok(false);
                };
                if !pattern_matches(pattern, value, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn key_span(_key: &str, _len: usize, fallback: Span) -> Span {
    fallback
}
