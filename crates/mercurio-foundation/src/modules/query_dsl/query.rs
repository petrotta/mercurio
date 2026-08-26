use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

use crate::kir::{KirDocument, KirElement};
use crate::model::metadata_annotations_named;

pub fn elements_with_metadata<'a>(
    document: &'a KirDocument,
    metadata_type: &str,
) -> Vec<&'a KirElement> {
    document
        .elements
        .iter()
        .filter(|element| {
            !metadata_annotations_named(&element.properties, metadata_type).is_empty()
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub source: QuerySource,
    pub filters: Vec<FilterExpr>,
    pub projections: Vec<Projection>,
    pub order_by: Option<OrderBy>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuerySource {
    Elements,
    Match { patterns: Vec<TriplePattern> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriplePattern {
    pub subject: TermPattern,
    pub predicate: String,
    pub object: TermPattern,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TermPattern {
    Variable(String),
    Literal(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterExpr {
    Equals { field: String, value: String },
    NotEquals { field: String, value: String },
    Contains { field: String, value: String },
    In { field: String, values: Vec<String> },
    Exists { field: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projection {
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBy {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    message: String,
}

impl QueryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QueryError {}

pub fn parse_query(input: &str) -> Result<Query, QueryError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(QueryError::new("query must not be empty"));
    }

    if starts_with_keyword(trimmed, "match") {
        return parse_match_query(trimmed);
    }

    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("from elements") {
        return Err(QueryError::new(
            "expected query to start with `from elements` or `match`",
        ));
    }

    let mut rest = trimmed["from elements".len()..].trim();
    let mut filters = Vec::new();

    while starts_with_keyword(rest, "where") {
        rest = rest["where".len()..].trim();
        let (where_text, remaining) = split_at_next_clause(rest, &["where", "select"])?;
        filters.push(parse_filter(where_text.trim())?);
        rest = remaining.trim();
    }

    if !starts_with_keyword(rest, "select") {
        return Err(QueryError::new("expected `select` clause"));
    }
    rest = rest["select".len()..].trim();
    let (select_text, remaining) = split_at_next_clause(rest, &["order by", "limit"])?;
    let projections = parse_projections(select_text.trim())?;
    let (order_by, limit) = parse_order_and_limit(remaining.trim())?;

    Ok(Query {
        source: QuerySource::Elements,
        filters,
        projections,
        order_by,
        limit,
    })
}

pub struct QueryEngine<'a> {
    document: &'a KirDocument,
}

impl<'a> QueryEngine<'a> {
    pub fn new(document: &'a KirDocument) -> Self {
        Self { document }
    }

    pub fn execute(&self, query: &Query) -> Result<QueryResultSet, QueryError> {
        match &query.source {
            QuerySource::Elements => self.execute_elements(query),
            QuerySource::Match { patterns } => self.execute_match(patterns, query),
        }
    }

    fn execute_elements(&self, query: &Query) -> Result<QueryResultSet, QueryError> {
        let columns = query
            .projections
            .iter()
            .map(|projection| projection.field.clone())
            .collect::<Vec<_>>();
        let mut rows = Vec::new();

        for element in &self.document.elements {
            if !query
                .filters
                .iter()
                .all(|filter| filter_matches(element, filter))
            {
                continue;
            }

            let row = query
                .projections
                .iter()
                .map(|projection| {
                    (
                        projection.field.clone(),
                        element_field_value(element, &projection.field).unwrap_or(Value::Null),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            rows.push(row);
        }

        sort_and_limit_rows(&mut rows, query.order_by.as_ref(), query.limit);
        Ok(QueryResultSet { columns, rows })
    }

    fn execute_match(
        &self,
        patterns: &[TriplePattern],
        query: &Query,
    ) -> Result<QueryResultSet, QueryError> {
        let columns = query
            .projections
            .iter()
            .map(|projection| projection.field.trim_start_matches('?').to_string())
            .collect::<Vec<_>>();
        let triples = self.triples();
        let elements_by_id = self.elements_by_id();
        let mut bindings = vec![BTreeMap::new()];

        for pattern in patterns {
            let mut next_bindings = Vec::new();
            for binding in &bindings {
                for triple in &triples {
                    if let Some(next) = bind_triple(pattern, triple, binding) {
                        next_bindings.push(next);
                    }
                }
            }
            bindings = next_bindings;
            if bindings.is_empty() {
                break;
            }
        }

        let mut rows = Vec::new();
        for binding in bindings {
            if !query
                .filters
                .iter()
                .all(|filter| binding_filter_matches(filter, &binding, &elements_by_id))
            {
                continue;
            }

            let row = query
                .projections
                .iter()
                .map(|projection| {
                    let column = projection.field.trim_start_matches('?').to_string();
                    let value = projection_value(&projection.field, &binding, &elements_by_id);
                    (column, value)
                })
                .collect::<BTreeMap<_, _>>();
            rows.push(row);
        }

        sort_and_limit_rows(&mut rows, query.order_by.as_ref(), query.limit);
        Ok(QueryResultSet { columns, rows })
    }

    fn triples(&self) -> Vec<Triple> {
        let mut triples = Vec::new();
        for element in &self.document.elements {
            push_triple_values(
                &mut triples,
                &element.id,
                "id",
                Value::String(element.id.clone()),
            );
            push_triple_values(
                &mut triples,
                &element.id,
                "kind",
                Value::String(element.kind.clone()),
            );
            push_triple_values(
                &mut triples,
                &element.id,
                "layer",
                Value::Number(Number::from(element.layer)),
            );
            for (predicate, value) in &element.properties {
                push_triple_values(&mut triples, &element.id, predicate, value.clone());
            }
        }
        triples
    }

    fn elements_by_id(&self) -> BTreeMap<&str, &KirElement> {
        self.document
            .elements
            .iter()
            .map(|element| (element.id.as_str(), element))
            .collect()
    }
}

#[derive(Debug, Clone)]
struct Triple {
    subject: String,
    predicate: String,
    object: Value,
}

fn parse_filter(input: &str) -> Result<FilterExpr, QueryError> {
    if input.is_empty() {
        return Err(QueryError::new("expected filter expression after `where`"));
    }

    if let Some(field) = input.strip_suffix(" exists") {
        return Ok(FilterExpr::Exists {
            field: parse_filter_field(field.trim())?,
        });
    }

    if let Some((field, value)) = split_once_keyword(input, "contains") {
        return Ok(FilterExpr::Contains {
            field: parse_filter_field(field.trim())?,
            value: parse_string_literal(value.trim())?,
        });
    }

    if let Some((field, values)) = split_once_keyword(input, "in") {
        return Ok(FilterExpr::In {
            field: parse_filter_field(field.trim())?,
            values: parse_string_list(values.trim())?,
        });
    }

    if let Some((field, value)) = input.split_once("!=") {
        return Ok(FilterExpr::NotEquals {
            field: parse_filter_field(field.trim())?,
            value: parse_string_literal(value.trim())?,
        });
    }

    let (field, value) = input.split_once('=').ok_or_else(|| {
        QueryError::new("expected filter of form `field = value`, `field != value`, `field contains value`, `field in [...]`, or `field exists`")
    })?;
    Ok(FilterExpr::Equals {
        field: parse_filter_field(field.trim())?,
        value: parse_string_literal(value.trim())?,
    })
}

fn parse_match_query(input: &str) -> Result<Query, QueryError> {
    let (match_text, remaining) = split_at_next_clause(input, &["where", "select"])?;
    let mut patterns = Vec::new();
    for clause in split_match_clauses(match_text) {
        patterns.push(parse_triple_pattern(clause)?);
    }
    if patterns.is_empty() {
        return Err(QueryError::new("expected at least one `match` pattern"));
    }

    let mut rest = remaining.trim();
    let mut filters = Vec::new();
    while starts_with_keyword(rest, "where") {
        rest = rest["where".len()..].trim();
        let (where_text, remaining) = split_at_next_clause(rest, &["where", "select"])?;
        filters.push(parse_filter(where_text.trim())?);
        rest = remaining.trim();
    }

    if !starts_with_keyword(rest, "select") {
        return Err(QueryError::new("expected `select` clause"));
    }
    rest = rest["select".len()..].trim();
    let (select_text, remaining) = split_at_next_clause(rest, &["order by", "limit"])?;
    let projections = parse_projections(select_text.trim())?;
    let (order_by, limit) = parse_order_and_limit(remaining.trim())?;

    Ok(Query {
        source: QuerySource::Match { patterns },
        filters,
        projections,
        order_by,
        limit,
    })
}

fn split_match_clauses(input: &str) -> Vec<&str> {
    let mut clauses = Vec::new();
    for chunk in input.split("match") {
        let clause = chunk.trim();
        if !clause.is_empty() {
            clauses.push(clause);
        }
    }
    clauses
}

fn parse_triple_pattern(input: &str) -> Result<TriplePattern, QueryError> {
    let tokens = tokenize_pattern(input)?;
    if tokens.len() != 3 {
        return Err(QueryError::new(format!(
            "expected match pattern of form `subject predicate object`, got `{input}`"
        )));
    }

    Ok(TriplePattern {
        subject: parse_term_pattern(&tokens[0])?,
        predicate: parse_field_name(&tokens[1])?,
        object: parse_term_pattern(&tokens[2])?,
    })
}

fn tokenize_pattern(input: &str) -> Result<Vec<String>, QueryError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.peek().copied() {
        if character.is_whitespace() {
            chars.next();
            continue;
        }

        if character == '"' {
            chars.next();
            let mut token = String::new();
            let mut terminated = false;
            for next in chars.by_ref() {
                if next == '"' {
                    terminated = true;
                    break;
                }
                token.push(next);
            }
            if !terminated {
                return Err(QueryError::new(
                    "unterminated string literal in match pattern",
                ));
            }
            tokens.push(format!("\"{token}\""));
            continue;
        }

        let mut token = String::new();
        while let Some(next) = chars.peek().copied() {
            if next.is_whitespace() {
                break;
            }
            token.push(next);
            chars.next();
        }
        tokens.push(token);
    }
    Ok(tokens)
}

fn parse_term_pattern(input: &str) -> Result<TermPattern, QueryError> {
    if let Some(variable) = input.strip_prefix('?') {
        let variable = parse_variable_name(variable)?;
        return Ok(TermPattern::Variable(variable));
    }
    Ok(TermPattern::Literal(parse_string_literal(input)?))
}

fn parse_variable_name(input: &str) -> Result<String, QueryError> {
    if input.is_empty() {
        return Err(QueryError::new("variable name must not be empty"));
    }
    if !input
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(QueryError::new(format!("invalid variable name: ?{input}")));
    }
    Ok(input.to_string())
}

fn parse_projections(input: &str) -> Result<Vec<Projection>, QueryError> {
    if input.is_empty() {
        return Err(QueryError::new("expected at least one selected field"));
    }

    input
        .split(',')
        .map(|field| {
            let field = field.trim();
            if let Some(variable) = field.strip_prefix('?') {
                parse_variable_projection(variable)?;
                return Ok(Projection {
                    field: field.to_string(),
                });
            }
            Ok(Projection {
                field: parse_field_name(field)?,
            })
        })
        .collect()
}

fn parse_variable_projection(input: &str) -> Result<(), QueryError> {
    if let Some((variable, field)) = input.split_once('.') {
        parse_variable_name(variable)?;
        parse_field_name(field)?;
        return Ok(());
    }
    parse_variable_name(input).map(|_| ())
}

fn parse_filter_field(input: &str) -> Result<String, QueryError> {
    if let Some(variable) = input.strip_prefix('?') {
        parse_variable_projection(variable)?;
        return Ok(input.to_string());
    }
    parse_field_name(input)
}

fn parse_field_name(input: &str) -> Result<String, QueryError> {
    if input.is_empty() {
        return Err(QueryError::new("field name must not be empty"));
    }
    if !input
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '.'))
    {
        return Err(QueryError::new(format!("invalid field name: {input}")));
    }
    Ok(input.to_string())
}

fn parse_order_and_limit(input: &str) -> Result<(Option<OrderBy>, Option<usize>), QueryError> {
    let mut rest = input.trim();
    let mut order_by = None;
    let mut limit = None;

    if starts_with_keyword(rest, "order by") {
        rest = rest["order by".len()..].trim();
        if rest.is_empty() {
            return Err(QueryError::new("expected field after `order by`"));
        }
        let (order_text, remaining) = split_at_next_clause(rest, &["limit"])?;
        order_by = Some(parse_order_by(order_text.trim())?);
        rest = remaining.trim();
    }

    if starts_with_keyword(rest, "limit") {
        rest = rest["limit".len()..].trim();
        if rest.is_empty() {
            return Err(QueryError::new("expected row count after `limit`"));
        }
        limit = Some(
            rest.parse::<usize>()
                .map_err(|_| QueryError::new("limit must be a non-negative integer"))?,
        );
        rest = "";
    }

    if !rest.trim().is_empty() {
        return Err(QueryError::new(format!(
            "unexpected query text: {}",
            rest.trim()
        )));
    }

    Ok((order_by, limit))
}

fn parse_order_by(input: &str) -> Result<OrderBy, QueryError> {
    let mut parts = input.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(QueryError::new("expected field after `order by`"));
    }

    let direction = match parts.last().copied() {
        Some(value) if value.eq_ignore_ascii_case("desc") => {
            parts.pop();
            SortDirection::Desc
        }
        Some(value) if value.eq_ignore_ascii_case("asc") => {
            parts.pop();
            SortDirection::Asc
        }
        _ => SortDirection::Asc,
    };
    if parts.len() != 1 {
        return Err(QueryError::new("expected `order by FIELD [asc|desc]`"));
    }
    let field = parts[0];
    if let Some(variable) = field.strip_prefix('?') {
        parse_variable_projection(variable)?;
    } else {
        parse_field_name(field)?;
    }

    Ok(OrderBy {
        field: field.trim_start_matches('?').to_string(),
        direction,
    })
}

fn parse_string_literal(input: &str) -> Result<String, QueryError> {
    let input = input.trim();
    if input.len() >= 2 && input.starts_with('"') && input.ends_with('"') {
        return Ok(input[1..input.len() - 1].to_string());
    }
    if input.is_empty() {
        return Err(QueryError::new("filter value must not be empty"));
    }
    Ok(input.to_string())
}

fn parse_string_list(input: &str) -> Result<Vec<String>, QueryError> {
    let input = input.trim();
    if !(input.starts_with('[') && input.ends_with(']')) {
        return Err(QueryError::new(
            "expected list literal like `[\"A\", \"B\"]`",
        ));
    }
    let inner = input[1..input.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|item| parse_string_literal(item.trim()))
        .collect()
}

fn filter_matches(element: &KirElement, filter: &FilterExpr) -> bool {
    match filter {
        FilterExpr::Equals { field, value } => {
            element_field_value(element, field).is_some_and(|actual| value_matches(&actual, value))
        }
        FilterExpr::NotEquals { field, value } => {
            element_field_value(element, field).is_some_and(|actual| !value_matches(&actual, value))
        }
        FilterExpr::Contains { field, value } => {
            element_field_value(element, field).is_some_and(|actual| value_contains(&actual, value))
        }
        FilterExpr::In { field, values } => {
            element_field_value(element, field).is_some_and(|actual| {
                values
                    .iter()
                    .any(|expected| value_matches(&actual, expected))
            })
        }
        FilterExpr::Exists { field } => element_field_value(element, field).is_some(),
    }
}

fn binding_filter_matches(
    filter: &FilterExpr,
    binding: &BTreeMap<String, Value>,
    elements_by_id: &BTreeMap<&str, &KirElement>,
) -> bool {
    match filter {
        FilterExpr::Equals { field, value } => {
            let actual = binding_field_value(field, binding, elements_by_id);
            actual.is_some_and(|actual| value_matches(&actual, value))
        }
        FilterExpr::NotEquals { field, value } => {
            let actual = binding_field_value(field, binding, elements_by_id);
            actual.is_some_and(|actual| !value_matches(&actual, value))
        }
        FilterExpr::Contains { field, value } => {
            let actual = binding_field_value(field, binding, elements_by_id);
            actual.is_some_and(|actual| value_contains(&actual, value))
        }
        FilterExpr::In { field, values } => {
            let actual = binding_field_value(field, binding, elements_by_id);
            actual.is_some_and(|actual| values.iter().any(|value| value_matches(&actual, value)))
        }
        FilterExpr::Exists { field } => {
            binding_field_value(field, binding, elements_by_id).is_some()
        }
    }
}

fn value_matches(actual: &Value, expected: &str) -> bool {
    match actual {
        Value::String(value) => value == expected,
        Value::Number(value) => value.to_string() == expected,
        Value::Bool(value) => value.to_string() == expected,
        Value::Null => expected == "null",
        _ => actual == &Value::String(expected.to_string()),
    }
}

fn value_contains(actual: &Value, expected: &str) -> bool {
    match actual {
        Value::String(value) => value.contains(expected),
        Value::Array(values) => values.iter().any(|value| value_contains(value, expected)),
        Value::Number(value) => value.to_string().contains(expected),
        Value::Bool(value) => value.to_string().contains(expected),
        Value::Null => false,
        Value::Object(_) => actual.to_string().contains(expected),
    }
}

fn projection_value(
    projection: &str,
    binding: &BTreeMap<String, Value>,
    elements_by_id: &BTreeMap<&str, &KirElement>,
) -> Value {
    let projection = projection.trim_start_matches('?');
    let Some((variable, field)) = projection.split_once('.') else {
        return binding.get(projection).cloned().unwrap_or(Value::Null);
    };
    let Some(Value::String(element_id)) = binding.get(variable) else {
        return Value::Null;
    };
    elements_by_id
        .get(element_id.as_str())
        .and_then(|element| element_field_value(element, field))
        .unwrap_or(Value::Null)
}

fn binding_field_value(
    field: &str,
    binding: &BTreeMap<String, Value>,
    elements_by_id: &BTreeMap<&str, &KirElement>,
) -> Option<Value> {
    if field.starts_with('?') {
        let value = projection_value(field, binding, elements_by_id);
        return (!value.is_null()).then_some(value);
    }
    binding.get(field).cloned()
}

fn sort_and_limit_rows(
    rows: &mut Vec<BTreeMap<String, Value>>,
    order_by: Option<&OrderBy>,
    limit: Option<usize>,
) {
    if let Some(order_by) = order_by {
        rows.sort_by(|left, right| {
            let left = left.get(&order_by.field).unwrap_or(&Value::Null);
            let right = right.get(&order_by.field).unwrap_or(&Value::Null);
            let ordering = query_value_sort_key(left).cmp(&query_value_sort_key(right));
            match order_by.direction {
                SortDirection::Asc => ordering,
                SortDirection::Desc => ordering.reverse(),
            }
        });
    }

    if let Some(limit) = limit {
        rows.truncate(limit);
    }
}

fn query_value_sort_key(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn bind_triple(
    pattern: &TriplePattern,
    triple: &Triple,
    binding: &BTreeMap<String, Value>,
) -> Option<BTreeMap<String, Value>> {
    if pattern.predicate != triple.predicate {
        return None;
    }

    let mut next = binding.clone();
    bind_term(
        &pattern.subject,
        &Value::String(triple.subject.clone()),
        &mut next,
    )?;
    bind_term(&pattern.object, &triple.object, &mut next)?;
    Some(next)
}

fn bind_term(
    pattern: &TermPattern,
    value: &Value,
    binding: &mut BTreeMap<String, Value>,
) -> Option<()> {
    match pattern {
        TermPattern::Literal(expected) => value_matches(value, expected).then_some(()),
        TermPattern::Variable(variable) => {
            if let Some(bound) = binding.get(variable) {
                return (bound == value).then_some(());
            }
            binding.insert(variable.clone(), value.clone());
            Some(())
        }
    }
}

fn push_triple_values(triples: &mut Vec<Triple>, subject: &str, predicate: &str, value: Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                triples.push(Triple {
                    subject: subject.to_string(),
                    predicate: predicate.to_string(),
                    object: value,
                });
            }
        }
        value => triples.push(Triple {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: value,
        }),
    }
}

fn element_field_value(element: &KirElement, field: &str) -> Option<Value> {
    let mut segments = field.split('.');
    let first = segments.next()?;
    let mut value = match first {
        "id" => Value::String(element.id.clone()),
        "kind" => Value::String(element.kind.clone()),
        "layer" => Value::Number(Number::from(element.layer)),
        "qualified_name" => element
            .properties
            .get(first)
            .cloned()
            .or_else(|| qualified_name_from_element_id(&element.id).map(Value::String))?,
        "metatype" => element.properties.get(first).cloned().or_else(|| {
            element
                .properties
                .get("metadata")
                .and_then(|metadata| metadata.get("metatype"))
                .cloned()
        })?,
        property => element.properties.get(property)?.clone(),
    };

    for segment in segments {
        value = match value {
            Value::Object(map) => map.get(segment)?.clone(),
            _ => return None,
        };
    }

    Some(value)
}

fn qualified_name_from_element_id(element_id: &str) -> Option<String> {
    let (_, qualified_name) = element_id.split_once('.')?;
    (!qualified_name.is_empty()).then(|| qualified_name.to_string())
}

fn split_at_next_clause<'a>(
    input: &'a str,
    clauses: &[&str],
) -> Result<(&'a str, &'a str), QueryError> {
    let mut next_index = None;
    for clause in clauses {
        if let Some(index) = find_keyword(input, clause) {
            next_index = Some(next_index.map_or(index, |current: usize| current.min(index)));
        }
    }

    Ok(match next_index {
        Some(index) => (&input[..index], &input[index..]),
        None => (input, ""),
    })
}

fn split_once_keyword<'a>(input: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let lower = input.to_ascii_lowercase();
    let needle = format!(" {keyword} ");
    lower.find(&needle).map(|index| {
        let value_index = index + needle.len();
        (&input[..index], &input[value_index..])
    })
}

fn starts_with_keyword(input: &str, keyword: &str) -> bool {
    let input = input.trim_start();
    input
        .get(..keyword.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(keyword))
        && input
            .as_bytes()
            .get(keyword.len())
            .is_none_or(|byte| byte.is_ascii_whitespace())
}

fn find_keyword(input: &str, keyword: &str) -> Option<usize> {
    let lower = input.to_ascii_lowercase();
    let needle = format!(" {keyword} ");
    lower.find(&needle).map(|index| index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kir::{KirDocument, KirElement};

    #[test]
    fn parses_basic_query() {
        let query = parse_query(
            r#"from elements where kind = "Model::Package" select id, qualified_name limit 2"#,
        )
        .unwrap();

        assert_eq!(query.projections.len(), 2);
        assert_eq!(query.limit, Some(2));
    }

    #[test]
    fn parses_multiple_where_order_and_not_equals() {
        let query = parse_query(
            r#"from elements where metatype contains "Requirement" where qualified_name != "Demo.Skip" select id, qualified_name order by qualified_name desc limit 5"#,
        )
        .unwrap();

        assert_eq!(query.filters.len(), 2);
        assert_eq!(query.limit, Some(5));
        assert_eq!(query.order_by.as_ref().unwrap().field, "qualified_name");
        assert_eq!(
            query.order_by.as_ref().unwrap().direction,
            SortDirection::Desc
        );
    }

    #[test]
    fn executes_property_filter_and_projection() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "pkg.Demo".to_string(),
                kind: "Model::Package".to_string(),
                layer: 2,
                properties: [(
                    "qualified_name".to_string(),
                    Value::String("Demo".to_string()),
                )]
                .into_iter()
                .collect(),
            }],
        };
        let query =
            parse_query(r#"from elements where qualified_name = "Demo" select id, missing"#)
                .unwrap();

        let result = QueryEngine::new(&document).execute(&query).unwrap();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0]["id"], "pkg.Demo");
        assert_eq!(result.rows[0]["missing"], Value::Null);
    }

    #[test]
    fn executes_match_patterns_with_array_relationships() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: [(
                        "features".to_string(),
                        serde_json::json!(["feature.Demo.Vehicle.mass"]),
                    )]
                    .into_iter()
                    .collect(),
                },
                KirElement {
                    id: "feature.Demo.Vehicle.mass".to_string(),
                    kind: "Model::AttributeUsage".to_string(),
                    layer: 2,
                    properties: Default::default(),
                },
            ],
        };
        let query = parse_query(
            r#"match ?type kind "Model::Systems::PartDefinition" match ?type features ?feature select ?type, ?feature"#,
        )
        .unwrap();

        let result = QueryEngine::new(&document).execute(&query).unwrap();

        assert_eq!(result.columns, vec!["type", "feature"]);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0]["type"], "type.Demo.Vehicle");
        assert_eq!(result.rows[0]["feature"], "feature.Demo.Vehicle.mass");
    }

    #[test]
    fn finds_elements_with_metadata() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "requirement.Demo.safeStart".to_string(),
                kind: "RequirementUsage".to_string(),
                layer: 2,
                properties: [(
                    "metadata".to_string(),
                    serde_json::json!({
                        "ReviewTag": {
                            "properties": {
                                "status": "draft"
                            }
                        }
                    }),
                )]
                .into_iter()
                .collect(),
            }],
        };

        let matches = elements_with_metadata(&document, "ReviewTag");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "requirement.Demo.safeStart");
    }

    #[test]
    fn filters_with_contains_and_in() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![
                KirElement {
                    id: "type.Demo.VehicleNeed".to_string(),
                    kind: "Model::Systems::RequirementDefinition".to_string(),
                    layer: 2,
                    properties: [(
                        "metatype".to_string(),
                        Value::String("Model::Systems::RequirementDefinition".to_string()),
                    )]
                    .into_iter()
                    .collect(),
                },
                KirElement {
                    id: "requirement.Demo.vehicleNeed".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: [(
                        "metatype".to_string(),
                        Value::String("Model::RequirementUsage".to_string()),
                    )]
                    .into_iter()
                    .collect(),
                },
            ],
        };
        let contains_query =
            parse_query(r#"from elements where metatype contains "Requirement" select id"#)
                .unwrap();
        let in_query =
            parse_query(r#"from elements where metatype in ["Model::RequirementUsage"] select id"#)
                .unwrap();

        let contains = QueryEngine::new(&document)
            .execute(&contains_query)
            .unwrap();
        let in_result = QueryEngine::new(&document).execute(&in_query).unwrap();

        assert_eq!(contains.rows.len(), 2);
        assert_eq!(in_result.rows.len(), 1);
        assert_eq!(in_result.rows[0]["id"], "requirement.Demo.vehicleNeed");
    }

    #[test]
    fn match_projection_can_dereference_bound_element_fields() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: [
                        (
                            "features".to_string(),
                            serde_json::json!(["feature.Demo.Vehicle.mass"]),
                        ),
                        (
                            "qualified_name".to_string(),
                            Value::String("Demo.Vehicle".to_string()),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                },
                KirElement {
                    id: "feature.Demo.Vehicle.mass".to_string(),
                    kind: "Model::AttributeUsage".to_string(),
                    layer: 2,
                    properties: [(
                        "qualified_name".to_string(),
                        Value::String("Demo.Vehicle.mass".to_string()),
                    )]
                    .into_iter()
                    .collect(),
                },
            ],
        };
        let query = parse_query(
            r#"match ?type features ?feature select ?type.qualified_name, ?feature.qualified_name"#,
        )
        .unwrap();

        let result = QueryEngine::new(&document).execute(&query).unwrap();

        assert_eq!(
            result.columns,
            vec!["type.qualified_name", "feature.qualified_name"]
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0]["type.qualified_name"], "Demo.Vehicle");
        assert_eq!(
            result.rows[0]["feature.qualified_name"],
            "Demo.Vehicle.mass"
        );
    }

    #[test]
    fn match_filters_can_use_bound_element_fields_and_order_results() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: [
                        (
                            "features".to_string(),
                            serde_json::json!([
                                "feature.Demo.Vehicle.mass",
                                "feature.Demo.Vehicle.cost"
                            ]),
                        ),
                        (
                            "qualified_name".to_string(),
                            Value::String("Demo.Vehicle".to_string()),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                },
                KirElement {
                    id: "feature.Demo.Vehicle.mass".to_string(),
                    kind: "Model::AttributeUsage".to_string(),
                    layer: 2,
                    properties: [
                        (
                            "qualified_name".to_string(),
                            Value::String("Demo.Vehicle.mass".to_string()),
                        ),
                        ("metatype".to_string(), Value::String("Mass".to_string())),
                    ]
                    .into_iter()
                    .collect(),
                },
                KirElement {
                    id: "feature.Demo.Vehicle.cost".to_string(),
                    kind: "Model::AttributeUsage".to_string(),
                    layer: 2,
                    properties: [
                        (
                            "qualified_name".to_string(),
                            Value::String("Demo.Vehicle.cost".to_string()),
                        ),
                        ("metatype".to_string(), Value::String("Cost".to_string())),
                    ]
                    .into_iter()
                    .collect(),
                },
            ],
        };
        let query = parse_query(
            r#"match ?type features ?feature where ?feature.metatype != "Cost" select ?feature.qualified_name order by ?feature.qualified_name desc"#,
        )
        .unwrap();

        let result = QueryEngine::new(&document).execute(&query).unwrap();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0]["feature.qualified_name"],
            "Demo.Vehicle.mass"
        );
    }
}
