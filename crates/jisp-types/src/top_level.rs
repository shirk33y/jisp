use std::collections::{BTreeMap, BTreeSet};

use jisp_ir::{Definition, Expr, ExprKind, Pattern, StringPart};

pub(crate) fn definition_groups(definitions: &[Definition]) -> Vec<Vec<usize>> {
    let names = definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| (definition.name.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let graph = definitions
        .iter()
        .map(|definition| definition_dependencies(&definition.value, &names))
        .collect::<Vec<_>>();

    let mut state = Tarjan::new(&graph);
    for index in 0..definitions.len() {
        if state.indices[index].is_none() {
            state.connect(index);
        }
    }
    state.groups
}

fn definition_dependencies(expr: &Expr, names: &BTreeMap<String, usize>) -> BTreeSet<usize> {
    let mut dependencies = BTreeSet::new();
    collect_expr(expr, names, &BTreeSet::new(), &mut dependencies);
    dependencies
}

fn collect_expr(
    expr: &Expr,
    names: &BTreeMap<String, usize>,
    bound: &BTreeSet<String>,
    dependencies: &mut BTreeSet<usize>,
) {
    match &expr.kind {
        ExprKind::Literal(_) => {}
        ExprKind::Name(name) => {
            if !bound.contains(name) {
                if let Some(index) = names.get(name) {
                    dependencies.insert(*index);
                }
            }
        }
        ExprKind::Lambda { params, rest, body } => {
            let mut scoped = bound.clone();
            scoped.extend(params.iter().cloned());
            if let Some(name) = rest {
                scoped.insert(name.clone());
            }
            collect_expr(body, names, &scoped, dependencies);
        }
        ExprKind::Let { bindings, body } => {
            let mut scoped = bound.clone();
            for (name, value) in bindings {
                collect_expr(value, names, &scoped, dependencies);
                scoped.insert(name.clone());
            }
            collect_expr(body, names, &scoped, dependencies);
        }
        ExprKind::Do(expressions)
        | ExprKind::And(expressions)
        | ExprKind::Or(expressions)
        | ExprKind::List(expressions) => {
            for expression in expressions {
                collect_expr(expression, names, bound, dependencies);
            }
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr(condition, names, bound, dependencies);
            collect_expr(then_branch, names, bound, dependencies);
            collect_expr(else_branch, names, bound, dependencies);
        }
        ExprKind::Not(expression) => {
            collect_expr(expression, names, bound, dependencies);
        }
        ExprKind::Call { callee, arguments } => {
            collect_expr(callee, names, bound, dependencies);
            for argument in arguments {
                collect_expr(argument, names, bound, dependencies);
            }
        }
        ExprKind::Object(fields) => {
            for (key, value) in fields {
                collect_expr(key, names, bound, dependencies);
                collect_expr(value, names, bound, dependencies);
            }
        }
        ExprKind::Field { object, key } => {
            collect_expr(object, names, bound, dependencies);
            collect_expr(key, names, bound, dependencies);
        }
        ExprKind::StringTemplate { parts, .. } => {
            for part in parts {
                match part {
                    StringPart::Literal(_) => {}
                    StringPart::Expr(expression) | StringPart::Splice(expression) => {
                        collect_expr(expression, names, bound, dependencies);
                    }
                }
            }
        }
        ExprKind::Case { subject, branches } => {
            collect_expr(subject, names, bound, dependencies);
            for branch in branches {
                let mut scoped = bound.clone();
                collect_pattern_bindings(&branch.pattern, &mut scoped);
                collect_expr(&branch.body, names, &scoped, dependencies);
            }
        }
    }
}

fn collect_pattern_bindings(pattern: &Pattern, bindings: &mut BTreeSet<String>) {
    match pattern {
        Pattern::Wildcard | Pattern::Literal(_) => {}
        Pattern::Bind(name) => {
            bindings.insert(name.clone());
        }
        Pattern::Alias { pattern, name } => {
            collect_pattern_bindings(pattern, bindings);
            bindings.insert(name.clone());
        }
        Pattern::Variant { fields, .. } => {
            for field in fields {
                collect_pattern_bindings(field, bindings);
            }
        }
        Pattern::List { prefix, rest } => {
            for item in prefix {
                collect_pattern_bindings(item, bindings);
            }
            if let Some(name) = rest {
                bindings.insert(name.clone());
            }
        }
        Pattern::Object(fields) => {
            for (_, field) in fields {
                collect_pattern_bindings(field, bindings);
            }
        }
    }
}

struct Tarjan<'a> {
    graph: &'a [BTreeSet<usize>],
    next_index: usize,
    indices: Vec<Option<usize>>,
    lowlink: Vec<usize>,
    stack: Vec<usize>,
    on_stack: Vec<bool>,
    groups: Vec<Vec<usize>>,
}

impl<'a> Tarjan<'a> {
    fn new(graph: &'a [BTreeSet<usize>]) -> Self {
        Self {
            graph,
            next_index: 0,
            indices: vec![None; graph.len()],
            lowlink: vec![0; graph.len()],
            stack: vec![],
            on_stack: vec![false; graph.len()],
            groups: vec![],
        }
    }

    fn connect(&mut self, node: usize) {
        self.indices[node] = Some(self.next_index);
        self.lowlink[node] = self.next_index;
        self.next_index += 1;
        self.stack.push(node);
        self.on_stack[node] = true;

        for dependency in &self.graph[node] {
            if self.indices[*dependency].is_none() {
                self.connect(*dependency);
                self.lowlink[node] = self.lowlink[node].min(self.lowlink[*dependency]);
            } else if self.on_stack[*dependency] {
                let dependency_index =
                    self.indices[*dependency].expect("stacked nodes have indices");
                self.lowlink[node] = self.lowlink[node].min(dependency_index);
            }
        }

        if self.lowlink[node] == self.indices[node].expect("connected nodes have indices") {
            let mut group = vec![];
            loop {
                let member = self.stack.pop().expect("root nodes are on the stack");
                self.on_stack[member] = false;
                group.push(member);
                if member == node {
                    break;
                }
            }
            group.sort_unstable();
            self.groups.push(group);
        }
    }
}
