use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use jisp_ir::{
    CaseBranch, Definition, Expr, ExprKind, Module, Pattern, StringPart, TypeDecl, VariantDecl,
};
use jisp_types::{Inferencer, ObjectRow, Scheme, Type, TypedModule};

use crate::{canonicalize, load_module, module_source_files, resolve_import, Error, TypeResolver};
use jisp_core::SourceMap;

pub(crate) fn infer_module_with_native_imports(
    sources: &mut SourceMap,
    path: &Path,
    module: Module,
) -> Result<(TypedModule, Vec<std::path::PathBuf>), Error> {
    let mut resolver = TypeResolver::new(sources);
    let imports = resolver.import_environments(path, &module)?;
    let imported = collect_imports(&mut resolver, path, &module, "")?;
    let main = Inferencer::with_prelude().infer_typed_module_with_imports(module, &imports)?;
    let dependencies = resolver.dependencies();

    Ok((merge_modules(imported, main), dependencies))
}

fn collect_imports(
    resolver: &mut TypeResolver<'_>,
    importer: &Path,
    module: &Module,
    prefix: &str,
) -> Result<Vec<TypedModule>, Error> {
    let mut modules = Vec::new();
    for import in &module.imports {
        let path = resolve_import(importer, &import.path)?;
        let import_prefix = join_prefix(prefix, &import.alias);
        let typed = infer_imported_module(resolver, &path)?;
        modules.extend(collect_imports(
            resolver,
            &path,
            &typed.module,
            &import_prefix,
        )?);
        modules.push(prefix_module(typed, &import_prefix));
    }
    Ok(modules)
}

fn infer_imported_module(
    resolver: &mut TypeResolver<'_>,
    path: &Path,
) -> Result<TypedModule, Error> {
    let key = canonicalize(path)?;
    for file in module_source_files(&key)? {
        resolver.dependencies.insert(file);
    }
    let module = load_module(resolver.sources, &key)?;
    let imports = resolver.import_environments(&key, &module)?;
    Ok(Inferencer::with_prelude().infer_typed_module_with_imports(module, &imports)?)
}

fn merge_modules(imported: Vec<TypedModule>, main: TypedModule) -> TypedModule {
    let mut module = Module {
        imports: vec![],
        types: vec![],
        definitions: vec![],
        exports: main.module.exports.clone(),
    };
    let mut schemes = BTreeMap::new();

    for imported in imported {
        module.types.extend(imported.module.types);
        module.definitions.extend(imported.module.definitions);
        schemes.extend(imported.schemes);
    }

    module.types.extend(main.module.types);
    module.definitions.extend(main.module.definitions);
    schemes.extend(main.schemes);

    TypedModule { module, schemes }
}

fn prefix_module(module: TypedModule, prefix: &str) -> TypedModule {
    let definition_names = module
        .module
        .definitions
        .iter()
        .map(|definition| definition.name.clone())
        .collect::<BTreeSet<_>>();
    let type_names = module
        .module
        .types
        .iter()
        .map(|decl| decl.name.clone())
        .collect::<BTreeSet<_>>();
    let variant_names = module
        .module
        .types
        .iter()
        .flat_map(|decl| decl.variants.iter().map(|variant| variant.name.clone()))
        .collect::<BTreeSet<_>>();
    let import_aliases = module
        .module
        .imports
        .iter()
        .map(|import| import.alias.clone())
        .collect::<BTreeSet<_>>();
    let type_name_map = type_names
        .iter()
        .map(|name| (name.clone(), join_prefix(prefix, name)))
        .collect::<BTreeMap<_, _>>();

    let rewriter = PrefixRewriter {
        prefix,
        definition_names: &definition_names,
        variant_names: &variant_names,
        import_aliases: &import_aliases,
        type_name_map: &type_name_map,
    };

    let types = module
        .module
        .types
        .iter()
        .map(|decl| rewriter.type_decl(decl))
        .collect();
    let definitions = module
        .module
        .definitions
        .iter()
        .map(|definition| rewriter.definition(definition))
        .collect();
    let schemes = module
        .schemes
        .iter()
        .map(|(name, scheme)| (join_prefix(prefix, name), rewriter.scheme(scheme)))
        .collect();

    TypedModule {
        module: Module {
            imports: vec![],
            types,
            definitions,
            exports: vec![],
        },
        schemes,
    }
}

struct PrefixRewriter<'a> {
    prefix: &'a str,
    definition_names: &'a BTreeSet<String>,
    variant_names: &'a BTreeSet<String>,
    import_aliases: &'a BTreeSet<String>,
    type_name_map: &'a BTreeMap<String, String>,
}

impl PrefixRewriter<'_> {
    fn definition(&self, definition: &Definition) -> Definition {
        Definition {
            name: join_prefix(self.prefix, &definition.name),
            public: false,
            value: self.expr(&definition.value, &BTreeSet::new()),
            span: definition.span,
        }
    }

    fn type_decl(&self, decl: &TypeDecl) -> TypeDecl {
        TypeDecl {
            name: join_prefix(self.prefix, &decl.name),
            variants: decl
                .variants
                .iter()
                .map(|variant| self.variant_decl(variant))
                .collect(),
            span: decl.span,
        }
    }

    fn variant_decl(&self, variant: &VariantDecl) -> VariantDecl {
        VariantDecl {
            name: join_prefix(self.prefix, &variant.name),
            field_types: variant
                .field_types
                .iter()
                .map(|field| rewrite_declared_type(field, self.type_name_map))
                .collect(),
            span: variant.span,
        }
    }

    fn expr(&self, expr: &Expr, bound: &BTreeSet<String>) -> Expr {
        Expr {
            kind: match &expr.kind {
                ExprKind::Literal(literal) => ExprKind::Literal(literal.clone()),
                ExprKind::Name(name) => ExprKind::Name(self.name(name, bound)),
                ExprKind::Lambda { params, rest, body } => {
                    let mut scoped = bound.clone();
                    scoped.extend(params.iter().cloned());
                    if let Some(rest) = rest {
                        scoped.insert(rest.clone());
                    }
                    ExprKind::Lambda {
                        params: params.clone(),
                        rest: rest.clone(),
                        body: Box::new(self.expr(body, &scoped)),
                    }
                }
                ExprKind::Let { bindings, body } => {
                    let mut scoped = bound.clone();
                    let mut rewritten = Vec::with_capacity(bindings.len());
                    for (name, value) in bindings {
                        rewritten.push((name.clone(), self.expr(value, &scoped)));
                        scoped.insert(name.clone());
                    }
                    ExprKind::Let {
                        bindings: rewritten,
                        body: Box::new(self.expr(body, &scoped)),
                    }
                }
                ExprKind::Do(expressions) => ExprKind::Do(self.exprs(expressions, bound)),
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => ExprKind::If {
                    condition: Box::new(self.expr(condition, bound)),
                    then_branch: Box::new(self.expr(then_branch, bound)),
                    else_branch: Box::new(self.expr(else_branch, bound)),
                },
                ExprKind::And(expressions) => ExprKind::And(self.exprs(expressions, bound)),
                ExprKind::Or(expressions) => ExprKind::Or(self.exprs(expressions, bound)),
                ExprKind::Not(expression) => ExprKind::Not(Box::new(self.expr(expression, bound))),
                ExprKind::Call { callee, arguments } => ExprKind::Call {
                    callee: Box::new(self.expr(callee, bound)),
                    arguments: self.exprs(arguments, bound),
                },
                ExprKind::List(items) => ExprKind::List(self.exprs(items, bound)),
                ExprKind::Object(fields) => ExprKind::Object(
                    fields
                        .iter()
                        .map(|(key, value)| (self.expr(key, bound), self.expr(value, bound)))
                        .collect(),
                ),
                ExprKind::Field { object, key } => ExprKind::Field {
                    object: Box::new(self.expr(object, bound)),
                    key: Box::new(self.expr(key, bound)),
                },
                ExprKind::StringTemplate { lines, parts } => ExprKind::StringTemplate {
                    lines: *lines,
                    parts: parts
                        .iter()
                        .map(|part| self.string_part(part, bound))
                        .collect(),
                },
                ExprKind::Case { subject, branches } => ExprKind::Case {
                    subject: Box::new(self.expr(subject, bound)),
                    branches: branches
                        .iter()
                        .map(|branch| self.case_branch(branch, bound))
                        .collect(),
                },
            },
            span: expr.span,
        }
    }

    fn exprs(&self, expressions: &[Expr], bound: &BTreeSet<String>) -> Vec<Expr> {
        expressions
            .iter()
            .map(|expression| self.expr(expression, bound))
            .collect()
    }

    fn string_part(&self, part: &StringPart, bound: &BTreeSet<String>) -> StringPart {
        match part {
            StringPart::Literal(value) => StringPart::Literal(value.clone()),
            StringPart::Expr(expr) => StringPart::Expr(self.expr(expr, bound)),
            StringPart::Splice(expr) => StringPart::Splice(self.expr(expr, bound)),
        }
    }

    fn case_branch(&self, branch: &CaseBranch, bound: &BTreeSet<String>) -> CaseBranch {
        let pattern = self.pattern(&branch.pattern);
        let mut scoped = bound.clone();
        collect_pattern_bindings(&pattern, &mut scoped);
        CaseBranch {
            pattern,
            body: self.expr(&branch.body, &scoped),
            span: branch.span,
        }
    }

    fn pattern(&self, pattern: &Pattern) -> Pattern {
        match pattern {
            Pattern::Wildcard => Pattern::Wildcard,
            Pattern::Bind(name) => Pattern::Bind(name.clone()),
            Pattern::Literal(literal) => Pattern::Literal(literal.clone()),
            Pattern::Variant { tag, fields } => Pattern::Variant {
                tag: self.variant_name(tag),
                fields: fields.iter().map(|field| self.pattern(field)).collect(),
            },
            Pattern::List { prefix, rest } => Pattern::List {
                prefix: prefix.iter().map(|field| self.pattern(field)).collect(),
                rest: rest.clone(),
            },
            Pattern::Object(fields) => Pattern::Object(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), self.pattern(value)))
                    .collect(),
            ),
        }
    }

    fn scheme(&self, scheme: &Scheme) -> Scheme {
        Scheme {
            variables: scheme.variables.clone(),
            body: self.ty(&scheme.body),
        }
    }

    fn ty(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(var) => Type::Var(*var),
            Type::Never => Type::Never,
            Type::Null => Type::Null,
            Type::Bool => Type::Bool,
            Type::Int => Type::Int,
            Type::Float => Type::Float,
            Type::Str => Type::Str,
            Type::List(item) => Type::List(Box::new(self.ty(item))),
            Type::Object(row) => Type::Object(ObjectRow {
                fields: row
                    .fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), self.ty(ty)))
                    .collect(),
                rest: row.rest,
            }),
            Type::Function {
                parameters,
                rest,
                result,
            } => Type::Function {
                parameters: parameters.iter().map(|ty| self.ty(ty)).collect(),
                rest: rest.as_ref().map(|ty| Box::new(self.ty(ty))),
                result: Box::new(self.ty(result)),
            },
            Type::Named { name, arguments } => Type::Named {
                name: self
                    .type_name_map
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone()),
                arguments: arguments.iter().map(|ty| self.ty(ty)).collect(),
            },
        }
    }

    fn name(&self, name: &str, bound: &BTreeSet<String>) -> String {
        if bound.contains(name) {
            name.to_owned()
        } else if self.definition_names.contains(name) || self.variant_names.contains(name) {
            join_prefix(self.prefix, name)
        } else {
            self.imported_name(name).unwrap_or_else(|| name.to_owned())
        }
    }

    fn variant_name(&self, name: &str) -> String {
        if self.variant_names.contains(name) {
            join_prefix(self.prefix, name)
        } else {
            self.imported_name(name).unwrap_or_else(|| name.to_owned())
        }
    }

    fn imported_name(&self, name: &str) -> Option<String> {
        let (alias, _) = name.split_once('.')?;
        if self.import_aliases.contains(alias) {
            Some(join_prefix(self.prefix, name))
        } else {
            None
        }
    }
}

fn collect_pattern_bindings(pattern: &Pattern, output: &mut BTreeSet<String>) {
    match pattern {
        Pattern::Bind(name) => {
            output.insert(name.clone());
        }
        Pattern::Variant { fields, .. } => {
            for field in fields {
                collect_pattern_bindings(field, output);
            }
        }
        Pattern::List { prefix, rest } => {
            for field in prefix {
                collect_pattern_bindings(field, output);
            }
            if let Some(rest) = rest {
                output.insert(rest.clone());
            }
        }
        Pattern::Object(fields) => {
            for (_, value) in fields {
                collect_pattern_bindings(value, output);
            }
        }
        Pattern::Wildcard | Pattern::Literal(_) => {}
    }
}

fn rewrite_declared_type(text: &str, names: &BTreeMap<String, String>) -> String {
    let mut output = String::new();
    let mut token = String::new();
    for ch in text.chars() {
        if is_declared_type_char(ch) {
            token.push(ch);
        } else {
            push_declared_type_token(&mut output, &mut token, names);
            output.push(ch);
        }
    }
    push_declared_type_token(&mut output, &mut token, names);
    output
}

fn push_declared_type_token(
    output: &mut String,
    token: &mut String,
    names: &BTreeMap<String, String>,
) {
    if token.is_empty() {
        return;
    }
    if let Some(rewritten) = names.get(token) {
        output.push_str(rewritten);
    } else {
        output.push_str(token);
    }
    token.clear();
}

fn is_declared_type_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '/' || ch == '.'
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}.{name}")
    }
}
