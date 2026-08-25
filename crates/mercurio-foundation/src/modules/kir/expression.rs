use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpressionIr {
    Literal {
        value: Value,
    },
    #[serde(rename = "self")]
    SelfRef,
    Tuple {
        items: Vec<ExpressionIr>,
    },
    Path {
        root: ExpressionPathRoot,
        segments: Vec<ExpressionPathSegment>,
    },
    Unary {
        op: UnaryExpressionOp,
        expr: Box<ExpressionIr>,
    },
    Binary {
        left: Box<ExpressionIr>,
        op: BinaryExpressionOp,
        right: Box<ExpressionIr>,
    },
    Call {
        function: String,
        args: Vec<ExpressionIr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpressionPathRoot {
    #[serde(rename = "self")]
    SelfRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpressionPathSegment {
    Resolved {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        feature: Option<String>,
    },
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnaryExpressionOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryExpressionOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

impl ExpressionPathRoot {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelfRef => "self",
        }
    }
}

impl ExpressionPathSegment {
    pub fn name(&self) -> &str {
        match self {
            Self::Resolved { name, .. } | Self::Name(name) => name,
        }
    }
}

#[derive(Debug)]
pub enum ExpressionIrError {
    MissingKind,
    UnsupportedKind(String),
    Invalid(String),
}

impl fmt::Display for ExpressionIrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKind => write!(f, "invalid expression_ir missing string kind"),
            Self::UnsupportedKind(kind) => {
                write!(f, "unsupported expression_ir kind `{kind}`")
            }
            Self::Invalid(message) => write!(f, "invalid expression_ir: {message}"),
        }
    }
}

impl std::error::Error for ExpressionIrError {}

#[derive(Debug)]
pub enum ExpressionValidationError {
    UnsupportedPathRoot(String),
    EmptyPath,
    UnsupportedFunction(String),
    InvalidFunctionArity { function: String, arity: usize },
}

impl fmt::Display for ExpressionValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPathRoot(root) => {
                write!(f, "unsupported expression_ir path root `{root}`")
            }
            Self::EmptyPath => write!(f, "expression_ir path must have at least one segment"),
            Self::UnsupportedFunction(function) => {
                write!(f, "unsupported expression_ir function `{function}`")
            }
            Self::InvalidFunctionArity { function, arity } => write!(
                f,
                "expression_ir function `{function}` expects one argument, got {arity}"
            ),
        }
    }
}

impl std::error::Error for ExpressionValidationError {}

impl ExpressionIr {
    pub fn from_value(value: &Value) -> Result<Self, ExpressionIrError> {
        let kind = value
            .get("kind")
            .and_then(Value::as_str)
            .ok_or(ExpressionIrError::MissingKind)?;
        if !matches!(
            kind,
            "literal" | "self" | "tuple" | "path" | "unary" | "binary" | "call"
        ) {
            return Err(ExpressionIrError::UnsupportedKind(kind.to_string()));
        }

        serde_json::from_value(normalize_expression_ir_value(value))
            .map_err(|err| ExpressionIrError::Invalid(err.to_string()))
    }

    pub fn to_value(&self) -> Result<Value, ExpressionIrError> {
        serde_json::to_value(self).map_err(|err| ExpressionIrError::Invalid(err.to_string()))
    }

    pub fn validate_runtime_supported(&self) -> Result<(), ExpressionValidationError> {
        match self {
            Self::Literal { .. } | Self::SelfRef => Ok(()),
            Self::Tuple { items } => {
                for item in items {
                    item.validate_runtime_supported()?;
                }
                Ok(())
            }
            Self::Path { root, segments } => {
                if *root != ExpressionPathRoot::SelfRef {
                    return Err(ExpressionValidationError::UnsupportedPathRoot(
                        root.as_str().to_string(),
                    ));
                }
                if segments.is_empty() {
                    return Err(ExpressionValidationError::EmptyPath);
                }
                Ok(())
            }
            Self::Unary { expr, .. } => expr.validate_runtime_supported(),
            Self::Binary { left, right, .. } => {
                left.validate_runtime_supported()?;
                right.validate_runtime_supported()
            }
            Self::Call { function, args } => {
                if !matches!(function.as_str(), "count" | "sum" | "min" | "max" | "avg") {
                    return Err(ExpressionValidationError::UnsupportedFunction(
                        function.clone(),
                    ));
                }
                if args.len() != 1 {
                    return Err(ExpressionValidationError::InvalidFunctionArity {
                        function: function.clone(),
                        arity: args.len(),
                    });
                }
                args[0].validate_runtime_supported()
            }
        }
    }

    pub fn render_constraint_expression(&self) -> String {
        match self {
            Self::Literal { value } => render_literal_value(value),
            Self::SelfRef => "self".to_string(),
            Self::Tuple { items } => format!(
                "({})",
                items
                    .iter()
                    .map(Self::render_constraint_expression)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Path { segments, .. } => render_path_segments(segments),
            Self::Unary { op, expr } => match op {
                UnaryExpressionOp::Negate => format!("-{}", expr.render_constraint_expression()),
                UnaryExpressionOp::Not => format!("not {}", expr.render_constraint_expression()),
            },
            Self::Binary { left, op, right } => format!(
                "({} {} {})",
                left.render_constraint_expression(),
                op.constraint_symbol(),
                right.render_constraint_expression()
            ),
            Self::Call { function, args } => format!(
                "{function}({})",
                args.iter()
                    .map(Self::render_constraint_expression)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    pub fn collect_path_variables(&self, output: &mut BTreeSet<String>) {
        match self {
            Self::Path { segments, .. } => {
                output.insert(render_path_segments(segments));
            }
            Self::Tuple { items } => {
                for item in items {
                    item.collect_path_variables(output);
                }
            }
            Self::Unary { expr, .. } => expr.collect_path_variables(output),
            Self::Binary { left, right, .. } => {
                left.collect_path_variables(output);
                right.collect_path_variables(output);
            }
            Self::Call { args, .. } => {
                for arg in args {
                    arg.collect_path_variables(output);
                }
            }
            Self::Literal { .. } | Self::SelfRef => {}
        }
    }

    pub fn evaluate(
        &self,
        context: &mut impl ExpressionEvaluationContext,
    ) -> Result<Value, ExpressionEvaluationError> {
        self.validate_runtime_supported()
            .map_err(|err| ExpressionEvaluationError::InvalidExpression(err.to_string()))?;
        match self {
            Self::Literal { value } => Ok(value.clone()),
            Self::SelfRef => Ok(Value::String(context.owner_id().to_string())),
            Self::Tuple { items } => {
                let values = items
                    .iter()
                    .map(|item| item.evaluate(context))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Array(values))
            }
            Self::Path { segments, .. } => {
                let values = context.resolve_path(segments)?;
                match values.as_slice() {
                    [value] => Ok(value.clone()),
                    _ => Ok(Value::Array(values)),
                }
            }
            Self::Unary { op, expr } => {
                let value = expr.evaluate(context)?;
                match op {
                    UnaryExpressionOp::Negate => {
                        let number = value_as_f64(&value, self)?;
                        Number::from_f64(-number).map(Value::Number).ok_or_else(|| {
                            ExpressionEvaluationError::UnsupportedAggregation {
                                expression: format!("{self:?}"),
                            }
                        })
                    }
                    UnaryExpressionOp::Not => {
                        let boolean = value.as_bool().ok_or_else(|| {
                            ExpressionEvaluationError::InvalidExpression(format!("{self:?}"))
                        })?;
                        Ok(Value::Bool(!boolean))
                    }
                }
            }
            Self::Binary { left, op, right } => {
                let left = left.evaluate(context)?;
                let right = right.evaluate(context)?;
                evaluate_binary_expression(*op, &left, &right, self)
            }
            Self::Call { function, args } => {
                if args.len() != 1 {
                    return Err(ExpressionEvaluationError::InvalidExpression(format!(
                        "{self:?}"
                    )));
                }

                let values = match args.first() {
                    Some(Self::Path { segments, .. }) => context.resolve_path(segments)?,
                    Some(arg) => vec![arg.evaluate(context)?],
                    None => {
                        return Err(ExpressionEvaluationError::InvalidExpression(format!(
                            "{self:?}"
                        )));
                    }
                };

                match function.as_str() {
                    "count" => Ok(Value::Number(Number::from(values.len() as u64))),
                    "sum" => number_from_f64(sum_numbers(values, context.owner_id(), self)?, self),
                    "min" => number_from_f64(min_number(values, context.owner_id(), self)?, self),
                    "max" => number_from_f64(max_number(values, context.owner_id(), self)?, self),
                    "avg" => number_from_f64(avg_numbers(values, context.owner_id(), self)?, self),
                    _ => Err(ExpressionEvaluationError::UnsupportedFunction {
                        function: function.clone(),
                        expression: format!("{self:?}"),
                    }),
                }
            }
        }
    }
}

impl BinaryExpressionOp {
    fn constraint_symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Power => "^",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::And => "and",
            Self::Or => "or",
        }
    }
}

fn normalize_expression_ir_value(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };

    let mut normalized = object.clone();
    match normalized.get("kind").and_then(Value::as_str) {
        Some("path") => {
            normalized
                .entry("root".to_string())
                .or_insert_with(|| Value::String("self".to_string()));
        }
        Some("tuple") => normalize_array_field(&mut normalized, "items"),
        Some("unary") => normalize_object_field(&mut normalized, "expr"),
        Some("binary") => {
            normalize_object_field(&mut normalized, "left");
            normalize_object_field(&mut normalized, "right");
        }
        Some("call") => normalize_array_field(&mut normalized, "args"),
        _ => {}
    }
    Value::Object(normalized)
}

fn normalize_object_field(object: &mut serde_json::Map<String, Value>, field: &str) {
    if let Some(value) = object.get(field).cloned() {
        object.insert(field.to_string(), normalize_expression_ir_value(&value));
    }
}

fn normalize_array_field(object: &mut serde_json::Map<String, Value>, field: &str) {
    if let Some(values) = object.get(field).and_then(Value::as_array) {
        object.insert(
            field.to_string(),
            Value::Array(values.iter().map(normalize_expression_ir_value).collect()),
        );
    }
}

fn render_literal_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn render_path_segments(segments: &[ExpressionPathSegment]) -> String {
    segments
        .iter()
        .map(ExpressionPathSegment::name)
        .collect::<Vec<_>>()
        .join(".")
}

pub trait ExpressionEvaluationContext {
    fn owner_id(&self) -> &str;

    fn resolve_path(
        &mut self,
        segments: &[ExpressionPathSegment],
    ) -> Result<Vec<Value>, ExpressionEvaluationError>;
}

#[derive(Debug)]
pub enum ExpressionEvaluationError {
    InvalidExpression(String),
    UnsupportedAggregation {
        expression: String,
    },
    UnsupportedFunction {
        function: String,
        expression: String,
    },
    NonNumericValue {
        owner: String,
        feature: String,
    },
}

impl fmt::Display for ExpressionEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExpression(expression) => write!(f, "invalid expression: {expression}"),
            Self::UnsupportedAggregation { expression } => {
                write!(f, "unsupported aggregation expression: {expression}")
            }
            Self::UnsupportedFunction {
                function,
                expression,
            } => write!(
                f,
                "unsupported expression_ir function `{function}`: {expression}"
            ),
            Self::NonNumericValue { owner, feature } => {
                write!(
                    f,
                    "non-numeric value encountered while reading {feature} from {owner}"
                )
            }
        }
    }
}

impl std::error::Error for ExpressionEvaluationError {}

fn evaluate_binary_expression(
    op: BinaryExpressionOp,
    left: &Value,
    right: &Value,
    expression: &ExpressionIr,
) -> Result<Value, ExpressionEvaluationError> {
    match op {
        BinaryExpressionOp::Add => numeric_binary(left, right, expression, |a, b| a + b),
        BinaryExpressionOp::Subtract => numeric_binary(left, right, expression, |a, b| a - b),
        BinaryExpressionOp::Multiply => numeric_binary(left, right, expression, |a, b| a * b),
        BinaryExpressionOp::Divide => numeric_binary(left, right, expression, |a, b| a / b),
        BinaryExpressionOp::Power => numeric_binary(left, right, expression, f64::powf),
        BinaryExpressionOp::Less => numeric_compare(left, right, expression, |a, b| a < b),
        BinaryExpressionOp::LessEqual => numeric_compare(left, right, expression, |a, b| a <= b),
        BinaryExpressionOp::Greater => numeric_compare(left, right, expression, |a, b| a > b),
        BinaryExpressionOp::GreaterEqual => numeric_compare(left, right, expression, |a, b| a >= b),
        BinaryExpressionOp::Equal => Ok(Value::Bool(left == right)),
        BinaryExpressionOp::NotEqual => Ok(Value::Bool(left != right)),
        BinaryExpressionOp::And => boolean_binary(left, right, expression, |a, b| a && b),
        BinaryExpressionOp::Or => boolean_binary(left, right, expression, |a, b| a || b),
    }
}

fn sum_numbers(
    values: Vec<Value>,
    owner_id: &str,
    expression: &ExpressionIr,
) -> Result<f64, ExpressionEvaluationError> {
    values.into_iter().try_fold(0.0_f64, |total, value| {
        Ok(total + aggregation_value_as_f64(value, owner_id, expression)?)
    })
}

fn min_number(
    values: Vec<Value>,
    owner_id: &str,
    expression: &ExpressionIr,
) -> Result<f64, ExpressionEvaluationError> {
    let mut numbers = values
        .into_iter()
        .map(|value| aggregation_value_as_f64(value, owner_id, expression));
    let Some(first) = numbers.next() else {
        return Err(ExpressionEvaluationError::UnsupportedAggregation {
            expression: format!("{expression:?}"),
        });
    };
    numbers.try_fold(first?, |minimum, value| Ok(minimum.min(value?)))
}

fn max_number(
    values: Vec<Value>,
    owner_id: &str,
    expression: &ExpressionIr,
) -> Result<f64, ExpressionEvaluationError> {
    let mut numbers = values
        .into_iter()
        .map(|value| aggregation_value_as_f64(value, owner_id, expression));
    let Some(first) = numbers.next() else {
        return Err(ExpressionEvaluationError::UnsupportedAggregation {
            expression: format!("{expression:?}"),
        });
    };
    numbers.try_fold(first?, |maximum, value| Ok(maximum.max(value?)))
}

fn avg_numbers(
    values: Vec<Value>,
    owner_id: &str,
    expression: &ExpressionIr,
) -> Result<f64, ExpressionEvaluationError> {
    if values.is_empty() {
        return Err(ExpressionEvaluationError::UnsupportedAggregation {
            expression: format!("{expression:?}"),
        });
    }
    let len = values.len() as f64;
    Ok(sum_numbers(values, owner_id, expression)? / len)
}

fn aggregation_value_as_f64(
    value: Value,
    owner_id: &str,
    expression: &ExpressionIr,
) -> Result<f64, ExpressionEvaluationError> {
    match value {
        Value::Number(number) => {
            number
                .as_f64()
                .ok_or_else(|| ExpressionEvaluationError::UnsupportedAggregation {
                    expression: format!("{expression:?}"),
                })
        }
        _ => Err(ExpressionEvaluationError::NonNumericValue {
            owner: owner_id.to_string(),
            feature: format!("{expression:?}"),
        }),
    }
}

fn number_from_f64(
    value: f64,
    expression: &ExpressionIr,
) -> Result<Value, ExpressionEvaluationError> {
    Number::from_f64(value).map(Value::Number).ok_or_else(|| {
        ExpressionEvaluationError::UnsupportedAggregation {
            expression: format!("{expression:?}"),
        }
    })
}

fn numeric_binary(
    left: &Value,
    right: &Value,
    expression: &ExpressionIr,
    op: impl FnOnce(f64, f64) -> f64,
) -> Result<Value, ExpressionEvaluationError> {
    let left = value_as_f64(left, expression)?;
    let right = value_as_f64(right, expression)?;
    Number::from_f64(op(left, right))
        .map(Value::Number)
        .ok_or_else(|| ExpressionEvaluationError::UnsupportedAggregation {
            expression: format!("{expression:?}"),
        })
}

fn numeric_compare(
    left: &Value,
    right: &Value,
    expression: &ExpressionIr,
    op: impl FnOnce(f64, f64) -> bool,
) -> Result<Value, ExpressionEvaluationError> {
    let left = value_as_f64(left, expression)?;
    let right = value_as_f64(right, expression)?;
    Ok(Value::Bool(op(left, right)))
}

fn boolean_binary(
    left: &Value,
    right: &Value,
    expression: &ExpressionIr,
    op: impl FnOnce(bool, bool) -> bool,
) -> Result<Value, ExpressionEvaluationError> {
    let left = left
        .as_bool()
        .ok_or_else(|| ExpressionEvaluationError::InvalidExpression(format!("{expression:?}")))?;
    let right = right
        .as_bool()
        .ok_or_else(|| ExpressionEvaluationError::InvalidExpression(format!("{expression:?}")))?;
    Ok(Value::Bool(op(left, right)))
}

fn value_as_f64(
    value: &Value,
    expression: &ExpressionIr,
) -> Result<f64, ExpressionEvaluationError> {
    value
        .as_f64()
        .ok_or_else(|| ExpressionEvaluationError::UnsupportedAggregation {
            expression: format!("{expression:?}"),
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use super::{
        BinaryExpressionOp, ExpressionEvaluationContext, ExpressionEvaluationError, ExpressionIr,
        ExpressionPathRoot, ExpressionPathSegment, UnaryExpressionOp,
    };

    #[derive(Default)]
    struct TestEvaluationContext {
        owner_id: String,
        paths: BTreeMap<Vec<String>, Vec<Value>>,
    }

    impl ExpressionEvaluationContext for TestEvaluationContext {
        fn owner_id(&self) -> &str {
            &self.owner_id
        }

        fn resolve_path(
            &mut self,
            segments: &[ExpressionPathSegment],
        ) -> Result<Vec<Value>, ExpressionEvaluationError> {
            let key = segments
                .iter()
                .map(ExpressionPathSegment::name)
                .map(str::to_string)
                .collect::<Vec<_>>();
            self.paths
                .get(&key)
                .cloned()
                .ok_or_else(|| ExpressionEvaluationError::InvalidExpression(key.join(".")))
        }
    }

    #[test]
    fn serializes_expression_ir_contract_shapes() {
        let expression = ExpressionIr::Binary {
            left: Box::new(ExpressionIr::Call {
                function: "sum".to_string(),
                args: vec![ExpressionIr::Path {
                    root: ExpressionPathRoot::SelfRef,
                    segments: vec![
                        ExpressionPathSegment::Resolved {
                            name: "parts".to_string(),
                            feature: Some("feature.Demo.vehicle.parts".to_string()),
                        },
                        ExpressionPathSegment::Resolved {
                            name: "mass".to_string(),
                            feature: Some("feature.Demo.Engine.mass".to_string()),
                        },
                    ],
                }],
            }),
            op: BinaryExpressionOp::Greater,
            right: Box::new(ExpressionIr::Unary {
                op: UnaryExpressionOp::Negate,
                expr: Box::new(ExpressionIr::Literal { value: json!(0) }),
            }),
        };

        assert_eq!(
            serde_json::to_value(expression).unwrap(),
            json!({
                "kind": "binary",
                "op": "greater",
                "left": {
                    "kind": "call",
                    "function": "sum",
                    "args": [{
                        "kind": "path",
                        "root": "self",
                        "segments": [
                            {"name": "parts", "feature": "feature.Demo.vehicle.parts"},
                            {"name": "mass", "feature": "feature.Demo.Engine.mass"}
                        ]
                    }]
                },
                "right": {
                    "kind": "unary",
                    "op": "negate",
                    "expr": {"kind": "literal", "value": 0}
                }
            })
        );
    }

    #[test]
    fn deserializes_legacy_string_path_segments() {
        let expression: ExpressionIr = serde_json::from_value(json!({
            "kind": "path",
            "root": "self",
            "segments": ["parts", "mass"]
        }))
        .unwrap();

        let ExpressionIr::Path { segments, .. } = expression else {
            panic!("expected path expression");
        };
        assert_eq!(
            segments
                .iter()
                .map(ExpressionPathSegment::name)
                .collect::<Vec<_>>(),
            vec!["parts", "mass"]
        );
    }

    #[test]
    fn rejects_unknown_expression_ir_kind() {
        let error = ExpressionIr::from_value(&json!({
            "kind": "select",
            "source": {"kind": "self"}
        }))
        .unwrap_err();

        assert_eq!(error.to_string(), "unsupported expression_ir kind `select`");
    }

    #[test]
    fn evaluates_pure_expression_ir_with_path_callback() {
        let mut context = TestEvaluationContext {
            owner_id: "assembly.Vehicle".to_string(),
            paths: [(
                vec!["parts".to_string(), "mass".to_string()],
                vec![json!(4.0), json!(6.5)],
            )]
            .into_iter()
            .collect(),
        };
        let expression = ExpressionIr::Binary {
            left: Box::new(ExpressionIr::Call {
                function: "sum".to_string(),
                args: vec![ExpressionIr::Path {
                    root: ExpressionPathRoot::SelfRef,
                    segments: vec![
                        ExpressionPathSegment::Name("parts".to_string()),
                        ExpressionPathSegment::Name("mass".to_string()),
                    ],
                }],
            }),
            op: BinaryExpressionOp::Greater,
            right: Box::new(ExpressionIr::Literal { value: json!(10) }),
        };

        assert_eq!(expression.evaluate(&mut context).unwrap(), json!(true));
    }

    #[test]
    fn reports_nonnumeric_sum_values_from_shared_evaluator() {
        let mut context = TestEvaluationContext {
            owner_id: "assembly.Vehicle".to_string(),
            paths: [(
                vec!["parts".to_string(), "mass".to_string()],
                vec![json!(4.0), json!("heavy")],
            )]
            .into_iter()
            .collect(),
        };
        let expression = ExpressionIr::Call {
            function: "sum".to_string(),
            args: vec![ExpressionIr::Path {
                root: ExpressionPathRoot::SelfRef,
                segments: vec![
                    ExpressionPathSegment::Name("parts".to_string()),
                    ExpressionPathSegment::Name("mass".to_string()),
                ],
            }],
        };

        let error = expression.evaluate(&mut context).unwrap_err();
        assert!(matches!(
            error,
            ExpressionEvaluationError::NonNumericValue {
                owner,
                ..
            } if owner == "assembly.Vehicle"
        ));
    }

    #[test]
    fn evaluates_numeric_aggregate_functions() {
        let mut context = TestEvaluationContext {
            owner_id: "assembly.Vehicle".to_string(),
            paths: [(
                vec!["parts".to_string(), "mass".to_string()],
                vec![json!(4.0), json!(6.0), json!(11.0)],
            )]
            .into_iter()
            .collect(),
        };
        let aggregate_arg = ExpressionIr::Path {
            root: ExpressionPathRoot::SelfRef,
            segments: vec![
                ExpressionPathSegment::Name("parts".to_string()),
                ExpressionPathSegment::Name("mass".to_string()),
            ],
        };

        for (function, expected) in [
            ("sum", json!(21.0)),
            ("min", json!(4.0)),
            ("max", json!(11.0)),
            ("avg", json!(7.0)),
        ] {
            let expression = ExpressionIr::Call {
                function: function.to_string(),
                args: vec![aggregate_arg.clone()],
            };
            assert_eq!(expression.evaluate(&mut context).unwrap(), expected);
        }
    }

    #[test]
    fn validates_runtime_supported_function_policy() {
        let expression = ExpressionIr::Call {
            function: "median".to_string(),
            args: vec![ExpressionIr::Literal { value: json!(1) }],
        };

        let error = expression.validate_runtime_supported().unwrap_err();
        assert_eq!(
            error.to_string(),
            "unsupported expression_ir function `median`"
        );
    }

    #[test]
    fn validates_runtime_supported_call_arity() {
        let expression = ExpressionIr::Call {
            function: "sum".to_string(),
            args: vec![
                ExpressionIr::Literal { value: json!(1) },
                ExpressionIr::Literal { value: json!(2) },
            ],
        };

        let error = expression.validate_runtime_supported().unwrap_err();
        assert_eq!(
            error.to_string(),
            "expression_ir function `sum` expects one argument, got 2"
        );
    }
}
