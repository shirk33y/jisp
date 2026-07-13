use std::collections::{BTreeMap, BTreeSet};

use jisp_ir::{CaseBranch, Expr, Literal, Pattern};

use super::{InferError, Inferencer, ObjectRow, Scheme, Type, UnifyError};

const MAX_OBJECT_COVERAGE_COMBINATIONS: usize = 256;

type ListLabels = Vec<String>;
type ListLabelSet = BTreeSet<ListLabels>;
type ObjectLabels = Vec<(String, String)>;
type ObjectLabelSet = BTreeSet<ObjectLabels>;

impl Inferencer {
    pub(super) fn infer_case(
        &mut self,
        subject: &Expr,
        branches: &[CaseBranch],
    ) -> Result<Type, InferError> {
        let subject_ty = self.infer_expr_located(subject)?;
        let result_ty = self.fresh_type();

        for branch in branches {
            let body_ty = self.with_scope(|inferencer| {
                let mut bindings = BTreeMap::new();
                inferencer.infer_pattern(&branch.pattern, subject_ty.clone(), &mut bindings)?;
                for (name, ty) in bindings {
                    inferencer.define(name, Scheme::mono(inferencer.apply(&ty)));
                }
                if let Some(guard) = &branch.guard {
                    let guard_ty = inferencer.infer_expr_located(guard)?;
                    inferencer.unify(guard_ty, Type::Bool)?;
                }
                inferencer.infer_expr_located(&branch.body)
            })?;
            self.unify(result_ty.clone(), body_ty)?;
        }

        self.check_case_exhaustive(&subject_ty, branches)?;
        Ok(result_ty)
    }

    fn check_case_exhaustive(
        &self,
        subject_ty: &Type,
        branches: &[CaseBranch],
    ) -> Result<(), InferError> {
        let subject_ty = self.apply(subject_ty);
        match &subject_ty {
            Type::Named { name, .. } => {
                let Some(variants) = self.type_variants.get(name) else {
                    return Ok(());
                };
                self.check_finite_case_exhaustive(name, variants.clone(), branches)
            }
            Type::Bool => self.check_finite_case_exhaustive(
                "bool",
                BTreeSet::from(["false".to_owned(), "true".to_owned()]),
                branches,
            ),
            Type::Null => self.check_finite_case_exhaustive(
                "null",
                BTreeSet::from(["null".to_owned()]),
                branches,
            ),
            Type::List(item) => self.check_list_case_exhaustive(item, branches),
            Type::Object(_) => self.check_object_case_exhaustive(&subject_ty, branches),
            _ if branches.is_empty() => Err(InferError::NonExhaustiveCase {
                type_name: subject_ty.to_string(),
                missing: vec!["_".to_owned()],
            }),
            _ => Ok(()),
        }
    }

    fn check_finite_case_exhaustive(
        &self,
        type_name: &str,
        expected: BTreeSet<String>,
        branches: &[CaseBranch],
    ) -> Result<(), InferError> {
        let mut covered = BTreeSet::new();
        let mut has_catch_all = false;
        for branch in branches {
            if branch.guard.is_some() {
                continue;
            }
            if has_catch_all {
                return Err(InferError::RedundantCasePattern(pattern_name(
                    &branch.pattern,
                )));
            }
            for pattern in coverage_patterns(&branch.pattern) {
                match pattern {
                    Pattern::Wildcard | Pattern::Bind(_) => has_catch_all = true,
                    Pattern::Variant { tag, .. } => {
                        if !covered.insert(tag.clone()) {
                            return Err(InferError::RedundantCasePattern(tag.clone()));
                        }
                    }
                    Pattern::Literal(Literal::Bool(value)) if type_name == "bool" => {
                        let name = value.to_string();
                        if !covered.insert(name.clone()) {
                            return Err(InferError::RedundantCasePattern(name));
                        }
                    }
                    Pattern::Literal(Literal::Null) if type_name == "null" => {
                        if !covered.insert("null".to_owned()) {
                            return Err(InferError::RedundantCasePattern("null".to_owned()));
                        }
                    }
                    Pattern::Literal(_) | Pattern::List { .. } | Pattern::Object(_) => {}
                    Pattern::Alias { .. } | Pattern::Or(_) => {
                        unreachable!("patterns are flattened above")
                    }
                }
            }
        }

        if has_catch_all {
            return Ok(());
        }

        let missing = expected.difference(&covered).cloned().collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(InferError::NonExhaustiveCase {
                type_name: type_name.to_owned(),
                missing,
            })
        }
    }

    fn check_list_case_exhaustive(
        &self,
        item: &Type,
        branches: &[CaseBranch],
    ) -> Result<(), InferError> {
        let mut exact_lengths = BTreeSet::new();
        let mut refined_exact_lengths: BTreeMap<usize, BTreeSet<Vec<String>>> = BTreeMap::new();
        let mut refined_exact_expected: BTreeMap<usize, BTreeSet<Vec<String>>> = BTreeMap::new();
        let mut rest_lengths = BTreeSet::new();
        let mut has_catch_all = false;

        for branch in branches {
            if branch.guard.is_some() {
                continue;
            }
            if has_catch_all {
                return Err(InferError::RedundantCasePattern(pattern_name(
                    &branch.pattern,
                )));
            }

            for pattern in coverage_patterns(&branch.pattern) {
                if has_catch_all {
                    return Err(InferError::RedundantCasePattern(pattern_name(pattern)));
                }
                match strip_alias(pattern) {
                    Pattern::Wildcard | Pattern::Bind(_) => has_catch_all = true,
                    Pattern::List { prefix, rest } => {
                        let irrefutable_prefix = prefix
                            .iter()
                            .all(|pattern| self.pattern_is_irrefutable_for_type(pattern, item));
                        let length = prefix.len();
                        if rest_lengths.iter().any(|covered| *covered <= length) {
                            return Err(InferError::RedundantCasePattern(pattern_name(
                                &branch.pattern,
                            )));
                        }

                        if rest.is_some() {
                            if !irrefutable_prefix {
                                continue;
                            }
                            rest_lengths.insert(length);
                            if length == 0 {
                                has_catch_all = true;
                            }
                        } else if irrefutable_prefix {
                            if refined_list_length_is_exhaustive(
                                length,
                                &refined_exact_lengths,
                                &refined_exact_expected,
                            ) {
                                return Err(InferError::RedundantCasePattern(pattern_name(
                                    &branch.pattern,
                                )));
                            }
                            if !exact_lengths.insert(length) {
                                return Err(InferError::RedundantCasePattern(pattern_name(
                                    &branch.pattern,
                                )));
                            }
                        } else if let Some((covered, expected)) =
                            self.list_pattern_refined_coverage(prefix, item)
                        {
                            if exact_lengths.contains(&length) {
                                return Err(InferError::RedundantCasePattern(pattern_name(
                                    &branch.pattern,
                                )));
                            }
                            let refined = refined_exact_lengths.entry(length).or_default();
                            if covered.iter().all(|item| refined.contains(item)) {
                                return Err(InferError::RedundantCasePattern(pattern_name(
                                    &branch.pattern,
                                )));
                            }
                            refined.extend(covered);
                            refined_exact_expected.insert(length, expected);
                        }
                    }
                    Pattern::Literal(_) | Pattern::Variant { .. } | Pattern::Object(_) => {}
                    Pattern::Or(_) | Pattern::Alias { .. } => {
                        unreachable!("patterns are flattened or aliases are stripped above")
                    }
                }
            }
        }

        if has_catch_all
            || list_coverage_is_exhaustive(
                &exact_lengths,
                &refined_exact_lengths,
                &refined_exact_expected,
                &rest_lengths,
            )
        {
            Ok(())
        } else {
            Err(InferError::NonExhaustiveCase {
                type_name: "list".to_owned(),
                missing: missing_list_patterns(
                    &exact_lengths,
                    &refined_exact_lengths,
                    &refined_exact_expected,
                    &rest_lengths,
                ),
            })
        }
    }

    fn check_object_case_exhaustive(
        &self,
        subject_ty: &Type,
        branches: &[CaseBranch],
    ) -> Result<(), InferError> {
        let mut has_catch_all = false;
        let mut refined_fields: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> =
            BTreeMap::new();
        let mut product_coverage = BTreeSet::new();
        let mut product_expected = None;

        for branch in branches {
            if branch.guard.is_some() {
                continue;
            }
            if has_catch_all || object_refinements_are_exhaustive(&refined_fields) {
                return Err(InferError::RedundantCasePattern(pattern_name(
                    &branch.pattern,
                )));
            }
            for pattern in coverage_patterns(&branch.pattern) {
                if has_catch_all || object_refinements_are_exhaustive(&refined_fields) {
                    return Err(InferError::RedundantCasePattern(pattern_name(pattern)));
                }
                if self.pattern_is_irrefutable_for_type(pattern, subject_ty) {
                    has_catch_all = true;
                    continue;
                }
                if let Some(coverage) = self.object_product_coverage(pattern, subject_ty) {
                    if coverage
                        .covered
                        .iter()
                        .all(|labels| product_coverage.contains(labels))
                    {
                        return Err(InferError::RedundantCasePattern(pattern_name(pattern)));
                    }
                    product_coverage.extend(coverage.covered);
                    product_expected = Some(coverage.expected);
                }
                if let Some(coverage) = self.pattern_finite_refinement_coverage(pattern, subject_ty)
                {
                    let entry = refined_fields
                        .entry(coverage.key)
                        .or_insert_with(|| (coverage.domain, BTreeSet::new()));
                    if coverage.labels.iter().all(|label| entry.1.contains(label)) {
                        return Err(InferError::RedundantCasePattern(pattern_name(pattern)));
                    }
                    entry.1.extend(coverage.labels);
                }
            }
        }

        if has_catch_all
            || object_refinements_are_exhaustive(&refined_fields)
            || product_expected
                .as_ref()
                .is_some_and(|expected| expected.is_subset(&product_coverage))
        {
            Ok(())
        } else {
            let missing = product_expected
                .as_ref()
                .map(|expected| missing_object_product_patterns(expected, &product_coverage))
                .filter(|missing| !missing.is_empty())
                .unwrap_or_else(|| vec!["object pattern".to_owned()]);
            Err(InferError::NonExhaustiveCase {
                type_name: "object".to_owned(),
                missing,
            })
        }
    }

    fn pattern_is_irrefutable_for_type(&self, pattern: &Pattern, ty: &Type) -> bool {
        match strip_alias(pattern) {
            Pattern::Wildcard | Pattern::Bind(_) => true,
            Pattern::Literal(Literal::Null) => matches!(self.apply(ty), Type::Null),
            Pattern::List {
                prefix,
                rest: Some(_),
            } => {
                let Type::List(item) = self.apply(ty) else {
                    return false;
                };
                prefix
                    .iter()
                    .all(|pattern| self.pattern_is_irrefutable_for_type(pattern, &item))
            }
            Pattern::Object(fields) => {
                let Type::Object(row) = self.apply(ty) else {
                    return false;
                };
                fields.iter().all(|(name, pattern)| {
                    row.fields
                        .get(name)
                        .is_some_and(|ty| self.pattern_is_irrefutable_for_type(pattern, ty))
                })
            }
            Pattern::Literal(_) | Pattern::Variant { .. } | Pattern::List { rest: None, .. } => {
                false
            }
            Pattern::Or(alternatives) => alternatives
                .iter()
                .any(|alternative| self.pattern_is_irrefutable_for_type(alternative, ty)),
            Pattern::Alias { .. } => unreachable!("aliases are stripped above"),
        }
    }

    fn list_pattern_refined_coverage(
        &self,
        prefix: &[Pattern],
        item: &Type,
    ) -> Option<(ListLabelSet, ListLabelSet)> {
        let domain = self.finite_domain_for_type(item)?;
        let expected = label_combinations(&domain, prefix.len(), None)?;
        let mut combinations = BTreeSet::from([Vec::new()]);

        for pattern in prefix {
            let labels = self.pattern_labels_for_domain(pattern, item, &domain)?;
            let mut next = BTreeSet::new();
            for prefix in &combinations {
                for label in &labels {
                    let mut item = prefix.clone();
                    item.push(label.clone());
                    next.insert(item);
                }
            }
            combinations = next;
        }

        Some((combinations, expected))
    }

    fn pattern_finite_refinement_coverage(
        &self,
        pattern: &Pattern,
        ty: &Type,
    ) -> Option<FiniteCoverage> {
        let domain = self.finite_domain_for_type(ty);
        if let Some(domain) = domain {
            let labels = self.pattern_labels_for_domain(pattern, ty, &domain)?;
            return Some(FiniteCoverage {
                key: String::new(),
                domain,
                labels,
            });
        }

        let Pattern::Object(fields) = strip_alias(pattern) else {
            return None;
        };
        let Type::Object(row) = self.apply(ty) else {
            return None;
        };

        for (name, pattern) in fields {
            let Some(field_ty) = row.fields.get(name) else {
                continue;
            };
            let Some(mut coverage) = self.pattern_finite_refinement_coverage(pattern, field_ty)
            else {
                continue;
            };
            let other_fields_are_irrefutable = fields.iter().all(|(other_name, other_pattern)| {
                other_name == name
                    || row.fields.get(other_name).is_some_and(|other_ty| {
                        self.pattern_is_irrefutable_for_type(other_pattern, other_ty)
                    })
            });
            if !other_fields_are_irrefutable {
                continue;
            }

            coverage.key = if coverage.key.is_empty() {
                name.clone()
            } else {
                format!("{name}.{}", coverage.key)
            };
            return Some(coverage);
        }

        None
    }

    fn object_product_coverage(&self, pattern: &Pattern, ty: &Type) -> Option<ProductCoverage> {
        let Pattern::Object(pattern_fields) = strip_alias(pattern) else {
            return None;
        };
        let Type::Object(row) = self.apply(ty) else {
            return None;
        };
        let patterns = pattern_fields
            .iter()
            .map(|(name, pattern)| (name, pattern))
            .collect::<BTreeMap<_, _>>();
        let mut combinations = BTreeSet::from([Vec::new()]);
        let mut expected = BTreeSet::from([Vec::new()]);
        let mut has_finite_field = false;

        for (name, field_ty) in &row.fields {
            match self.finite_domain_for_type(field_ty) {
                Some(domain) => {
                    has_finite_field = true;
                    expected = append_object_product_labels(
                        &expected,
                        name,
                        &domain,
                        Some(MAX_OBJECT_COVERAGE_COMBINATIONS),
                    )?;
                    let labels = patterns.get(name).map_or_else(
                        || Some(domain.clone()),
                        |pattern| self.pattern_labels_for_domain(pattern, field_ty, &domain),
                    )?;
                    let mut next = BTreeSet::new();
                    for prefix in &combinations {
                        for label in &labels {
                            let mut labels = prefix.clone();
                            labels.push((name.clone(), label.clone()));
                            next.insert(labels);
                            if next.len() > MAX_OBJECT_COVERAGE_COMBINATIONS {
                                return None;
                            }
                        }
                    }
                    combinations = next;
                }
                None => {
                    if patterns.get(name).is_some_and(|pattern| {
                        !self.pattern_is_irrefutable_for_type(pattern, field_ty)
                    }) {
                        return None;
                    }
                }
            }
        }
        has_finite_field.then_some(ProductCoverage {
            covered: combinations,
            expected,
        })
    }

    fn finite_domain_for_type(&self, ty: &Type) -> Option<BTreeSet<String>> {
        match self.apply(ty) {
            Type::Bool => Some(BTreeSet::from(["false".to_owned(), "true".to_owned()])),
            Type::Null => Some(BTreeSet::from(["null".to_owned()])),
            Type::Named { name, .. } => self.type_variants.get(&name).cloned(),
            _ => None,
        }
    }

    fn pattern_labels_for_domain(
        &self,
        pattern: &Pattern,
        ty: &Type,
        domain: &BTreeSet<String>,
    ) -> Option<BTreeSet<String>> {
        match strip_alias(pattern) {
            Pattern::Or(alternatives) => {
                let mut labels = BTreeSet::new();
                for alternative in alternatives {
                    labels.extend(self.pattern_labels_for_domain(alternative, ty, domain)?);
                }
                Some(labels)
            }
            Pattern::Wildcard | Pattern::Bind(_) => Some(domain.clone()),
            Pattern::Literal(Literal::Bool(value)) if matches!(self.apply(ty), Type::Bool) => {
                let label = value.to_string();
                domain.contains(&label).then(|| BTreeSet::from([label]))
            }
            Pattern::Literal(Literal::Null) if matches!(self.apply(ty), Type::Null) => domain
                .contains("null")
                .then(|| BTreeSet::from(["null".to_owned()])),
            Pattern::Variant { tag, fields }
                if fields.iter().all(pattern_is_always_irrefutable) =>
            {
                domain.contains(tag).then(|| BTreeSet::from([tag.clone()]))
            }
            _ => None,
        }
    }

    fn infer_pattern(
        &mut self,
        pattern: &Pattern,
        expected: Type,
        bindings: &mut BTreeMap<String, Type>,
    ) -> Result<(), InferError> {
        match pattern {
            Pattern::Wildcard => {}
            Pattern::Bind(name) => self.bind_pattern_name(name, expected, bindings)?,
            Pattern::Alias { pattern, name } => {
                self.infer_pattern(pattern, expected.clone(), bindings)?;
                self.bind_pattern_name(name, expected, bindings)?;
            }
            Pattern::Or(alternatives) => {
                let mut shared: Option<BTreeMap<String, Type>> = None;
                for alternative in alternatives {
                    let mut alternative_bindings = BTreeMap::new();
                    self.infer_pattern(alternative, expected.clone(), &mut alternative_bindings)?;
                    if let Some(first) = &shared {
                        if first.keys().collect::<Vec<_>>()
                            != alternative_bindings.keys().collect::<Vec<_>>()
                        {
                            return Err(InferError::InconsistentAlternativeBindings);
                        }
                        for (name, ty) in first {
                            self.unify(ty.clone(), alternative_bindings[name].clone())?;
                        }
                    } else {
                        shared = Some(alternative_bindings);
                    }
                }
                for (name, ty) in shared.expect("or patterns are non-empty after lowering") {
                    self.bind_pattern_name(&name, ty, bindings)?;
                }
            }
            Pattern::Literal(literal) => {
                let literal_ty = self.infer_literal(literal);
                self.unify(expected, literal_ty)?;
            }
            Pattern::Variant { tag, fields } => {
                let constructor_ty = self.lookup(tag)?;
                match (fields.as_slice(), self.apply(&constructor_ty)) {
                    ([], constructor_ty @ Type::Named { .. }) => {
                        self.unify(expected, constructor_ty)?;
                    }
                    (
                        fields,
                        Type::Function {
                            parameters,
                            rest: None,
                            result,
                        },
                    ) => {
                        if fields.len() != parameters.len() {
                            return Err(InferError::Unify(UnifyError::Arity {
                                left: parameters.len(),
                                right: fields.len(),
                            }));
                        }
                        self.unify(expected, *result)?;
                        for (field, parameter) in fields.iter().zip(parameters) {
                            self.infer_pattern(field, parameter, bindings)?;
                        }
                    }
                    ([], other) => {
                        self.unify(expected, other)?;
                    }
                    (_, other) => {
                        return Err(InferError::Unify(UnifyError::Mismatch {
                            left: other,
                            right: Type::Function {
                                parameters: fields.iter().map(|_| self.fresh_type()).collect(),
                                rest: None,
                                result: Box::new(expected),
                            },
                        }));
                    }
                }
            }
            Pattern::List { prefix, rest } => {
                let item = self.fresh_type();
                self.unify(expected, Type::List(Box::new(item.clone())))?;
                for pattern in prefix {
                    self.infer_pattern(pattern, item.clone(), bindings)?;
                }
                if let Some(name) = rest {
                    self.bind_pattern_name(name, Type::List(Box::new(item)), bindings)?;
                }
            }
            Pattern::Object(fields) => {
                let mut row_fields = BTreeMap::new();
                for (name, pattern) in fields {
                    let field_ty = self.fresh_type();
                    row_fields.insert(name.clone(), field_ty.clone());
                    self.infer_pattern(pattern, field_ty, bindings)?;
                }
                let rest = self.fresh_var();
                self.unify(
                    expected,
                    Type::Object(ObjectRow {
                        fields: row_fields,
                        rest: Some(rest),
                    }),
                )?;
            }
        }
        Ok(())
    }

    fn bind_pattern_name(
        &mut self,
        name: &str,
        ty: Type,
        bindings: &mut BTreeMap<String, Type>,
    ) -> Result<(), InferError> {
        if bindings.contains_key(name) {
            return Err(InferError::DuplicatePatternBinding(name.to_owned()));
        }
        bindings.insert(name.to_owned(), self.apply(&ty));
        Ok(())
    }
}

fn pattern_name(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_owned(),
        Pattern::Bind(name) => name.clone(),
        Pattern::Alias { pattern, name } => format!("{} as {name}", pattern_name(pattern)),
        Pattern::Or(_) => "or pattern".to_owned(),
        Pattern::Literal(Literal::Null) => "null".to_owned(),
        Pattern::Literal(Literal::Bool(value)) => value.to_string(),
        Pattern::Literal(Literal::Int(value)) => value.to_string(),
        Pattern::Literal(Literal::Float(value)) => value.to_string(),
        Pattern::Literal(Literal::String(value)) => format!("{value:?}"),
        Pattern::Variant { tag, .. } => tag.clone(),
        Pattern::List { .. } => "list pattern".to_owned(),
        Pattern::Object(_) => "object pattern".to_owned(),
    }
}

struct FiniteCoverage {
    key: String,
    domain: BTreeSet<String>,
    labels: BTreeSet<String>,
}

struct ProductCoverage {
    covered: ObjectLabelSet,
    expected: ObjectLabelSet,
}

fn object_refinements_are_exhaustive(
    refined_fields: &BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)>,
) -> bool {
    refined_fields
        .values()
        .any(|(domain, covered)| domain.is_subset(covered))
}

fn pattern_is_always_irrefutable(pattern: &Pattern) -> bool {
    matches!(strip_alias(pattern), Pattern::Wildcard | Pattern::Bind(_))
}

fn strip_alias(mut pattern: &Pattern) -> &Pattern {
    while let Pattern::Alias { pattern: inner, .. } = pattern {
        pattern = inner;
    }
    pattern
}

fn coverage_patterns(pattern: &Pattern) -> Vec<&Pattern> {
    match strip_alias(pattern) {
        Pattern::Or(alternatives) => alternatives.iter().flat_map(coverage_patterns).collect(),
        pattern => vec![pattern],
    }
}

fn list_coverage_is_exhaustive(
    exact_lengths: &BTreeSet<usize>,
    refined_exact_lengths: &BTreeMap<usize, BTreeSet<Vec<String>>>,
    refined_exact_expected: &BTreeMap<usize, BTreeSet<Vec<String>>>,
    rest_lengths: &BTreeSet<usize>,
) -> bool {
    let Some(rest_start) = rest_lengths.first().copied() else {
        return false;
    };
    (0..rest_start).all(|length| {
        exact_lengths.contains(&length)
            || refined_list_length_is_exhaustive(
                length,
                refined_exact_lengths,
                refined_exact_expected,
            )
    })
}

fn missing_list_patterns(
    exact_lengths: &BTreeSet<usize>,
    refined_exact_lengths: &BTreeMap<usize, BTreeSet<Vec<String>>>,
    refined_exact_expected: &BTreeMap<usize, BTreeSet<Vec<String>>>,
    rest_lengths: &BTreeSet<usize>,
) -> Vec<String> {
    if let Some(rest_start) = rest_lengths.first().copied() {
        let mut missing = vec![];
        for length in 0..rest_start {
            if exact_lengths.contains(&length) {
                continue;
            }
            let refined_missing = missing_refined_list_patterns(
                length,
                refined_exact_lengths,
                refined_exact_expected,
            );
            if refined_missing.is_empty() {
                missing.push(list_length_pattern(length));
            } else {
                missing.extend(refined_missing);
            }
        }
        return missing;
    }

    let max_exact = exact_lengths
        .iter()
        .chain(refined_exact_lengths.keys())
        .copied()
        .max()
        .unwrap_or(0);
    let mut missing = (0..=max_exact)
        .flat_map(|length| {
            if exact_lengths.contains(&length) {
                vec![]
            } else {
                let refined_missing = missing_refined_list_patterns(
                    length,
                    refined_exact_lengths,
                    refined_exact_expected,
                );
                if refined_missing.is_empty() {
                    vec![list_length_pattern(length)]
                } else {
                    refined_missing
                }
            }
        })
        .collect::<Vec<_>>();
    missing.push(format!("list length >= {}", max_exact + 1));
    missing
}

fn refined_list_length_is_exhaustive(
    length: usize,
    refined_exact_lengths: &BTreeMap<usize, BTreeSet<Vec<String>>>,
    refined_exact_expected: &BTreeMap<usize, BTreeSet<Vec<String>>>,
) -> bool {
    refined_exact_lengths
        .get(&length)
        .zip(refined_exact_expected.get(&length))
        .is_some_and(|(covered, expected)| expected.is_subset(covered))
}

fn missing_refined_list_patterns(
    length: usize,
    refined_exact_lengths: &BTreeMap<usize, BTreeSet<Vec<String>>>,
    refined_exact_expected: &BTreeMap<usize, BTreeSet<Vec<String>>>,
) -> Vec<String> {
    let Some(expected) = refined_exact_expected.get(&length) else {
        return vec![];
    };
    let covered = refined_exact_lengths.get(&length);
    expected
        .iter()
        .filter(|labels| covered.is_none_or(|covered| !covered.contains(*labels)))
        .map(|labels| format!("list [{}]", labels.join(", ")))
        .collect()
}

fn list_length_pattern(length: usize) -> String {
    if length == 0 {
        "[]".to_owned()
    } else {
        format!("list length {length}")
    }
}

fn label_combinations(
    domain: &BTreeSet<String>,
    length: usize,
    limit: Option<usize>,
) -> Option<ListLabelSet> {
    let mut combinations = BTreeSet::from([Vec::new()]);
    for _ in 0..length {
        let mut next = BTreeSet::new();
        for prefix in &combinations {
            for label in domain {
                let mut labels = prefix.clone();
                labels.push(label.clone());
                next.insert(labels);
                if limit.is_some_and(|limit| next.len() > limit) {
                    return None;
                }
            }
        }
        combinations = next;
    }
    Some(combinations)
}

fn append_object_product_labels(
    combinations: &ObjectLabelSet,
    field: &str,
    domain: &BTreeSet<String>,
    limit: Option<usize>,
) -> Option<ObjectLabelSet> {
    let mut next = BTreeSet::new();
    for prefix in combinations {
        for label in domain {
            let mut labels = prefix.clone();
            labels.push((field.to_owned(), label.clone()));
            next.insert(labels);
            if limit.is_some_and(|limit| next.len() > limit) {
                return None;
            }
        }
    }
    Some(next)
}

fn missing_object_product_patterns(
    expected: &ObjectLabelSet,
    covered: &ObjectLabelSet,
) -> Vec<String> {
    expected
        .iter()
        .filter(|labels| !covered.contains(*labels))
        .map(|labels| {
            let fields = labels
                .iter()
                .map(|(name, label)| format!("{name}: {label}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("object {{{fields}}}")
        })
        .collect()
}
