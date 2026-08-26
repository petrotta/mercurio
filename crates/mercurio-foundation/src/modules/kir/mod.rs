//! Kernel intermediate representation (KIR) persistence contract.
//!
//! KIR is the stable interchange boundary between source frontends, model
//! graph construction, runtime evaluation, and view generation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub mod expression;

pub use expression::{
    BinaryExpressionOp, ExpressionEvaluationContext, ExpressionEvaluationError, ExpressionIr,
    ExpressionIrError, ExpressionPathRoot, ExpressionPathSegment, ExpressionValidationError,
    UnaryExpressionOp,
};

pub const KIR_SCHEMA_VERSION: &str = "0.4";
pub const KIR_SCHEMA_VERSION_METADATA_KEY: &str = "kir_schema_version";
pub const SUPPORTED_KIR_SCHEMA_VERSIONS: &[&str] = &[KIR_SCHEMA_VERSION];
pub const KIR_PROP_MEMBERS: &str = "members";
pub const KIR_PROP_NAME: &str = "declared_name";
pub const KIR_PROP_OWNER: &str = "owner";
pub const KIR_PROP_SPECIALIZES: &str = "specializes";
pub const KIR_PROP_TYPE: &str = "type";
pub const REPRESENTATIVE_KIR_JSON: &str = r#"{
  "metadata": {
    "kir_schema_version": "0.4",
    "name": "Representative Foundation KIR"
  },
  "elements": [
    {
      "id": "pkg.Example",
      "kind": "model.Package",
      "layer": 2,
      "properties": {
        "qualified_name": "Example",
        "declared_name": "Example",
        "members": ["activity.Example.Startup"]
      }
    },
    {
      "id": "activity.Example.Startup",
      "kind": "model.Activity",
      "layer": 2,
      "properties": {
        "qualified_name": "Example.Startup",
        "declared_name": "Startup",
        "owner": "pkg.Example"
      }
    }
  ]
}"#;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KirDocument {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
    pub elements: Vec<KirElement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KirElement {
    pub id: String,
    pub kind: String,
    pub layer: u8,
    pub properties: BTreeMap<String, Value>,
}

impl Serialize for KirElement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut element = serializer.serialize_struct("KirElement", 3)?;
        element.serialize_field("id", &self.id)?;
        element.serialize_field("kind", &self.kind)?;
        if !self.properties.is_empty() {
            element.serialize_field("properties", &self.properties)?;
        }
        element.end()
    }
}

impl<'de> Deserialize<'de> for KirElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Id,
            Kind,
            Layer,
            Properties,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl Visitor<'_> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("`id`, `kind`, `layer`, or `properties`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "id" => Ok(Field::Id),
                            "kind" => Ok(Field::Kind),
                            "layer" => Ok(Field::Layer),
                            "properties" => Ok(Field::Properties),
                            other => Err(de::Error::unknown_field(
                                other,
                                &["id", "kind", "layer", "properties"],
                            )),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct ElementVisitor;

        impl<'de> Visitor<'de> for ElementVisitor {
            type Value = KirElement;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a KIR element")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut id = None;
                let mut kind = None;
                let mut layer = None;
                let mut properties: Option<BTreeMap<String, Value>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Id => {
                            if id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            id = Some(map.next_value()?);
                        }
                        Field::Kind => {
                            if kind.is_some() {
                                return Err(de::Error::duplicate_field("kind"));
                            }
                            kind = Some(map.next_value()?);
                        }
                        Field::Layer => {
                            if layer.is_some() {
                                return Err(de::Error::duplicate_field("layer"));
                            }
                            layer = Some(map.next_value()?);
                        }
                        Field::Properties => {
                            if properties.is_some() {
                                return Err(de::Error::duplicate_field("properties"));
                            }
                            properties = Some(map.next_value()?);
                        }
                    }
                }

                let id: String = id.ok_or_else(|| de::Error::missing_field("id"))?;
                let kind: String = kind.ok_or_else(|| de::Error::missing_field("kind"))?;
                let properties = properties.unwrap_or_default();
                let layer = layer.unwrap_or_else(|| inferred_layer(&id, &kind, &properties));
                Ok(KirElement {
                    id,
                    kind,
                    layer,
                    properties,
                })
            }
        }

        deserializer.deserialize_struct(
            "KirElement",
            &["id", "kind", "layer", "properties"],
            ElementVisitor,
        )
    }
}

#[derive(Debug)]
pub enum KirError {
    Io(std::io::Error),
    Json(serde_json::Error),
    DuplicateId(String),
    Validation(Vec<Diagnostic>),
    Frontend(String),
    Model(String),
}

/// Canonical diagnostic severity shared across every Mercurio diagnostic type.
///
/// Single-word variants serialize identically under `snake_case`, `lowercase`,
/// and `camelCase`, so this is wire-compatible with the per-crate severity
/// enums it replaces. Ordering is `Info < Warning < Error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// A resolved source location: a file plus a line/column range. Canonical home
/// for what was duplicated as `SourceSpanRef` in higher crates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceSpanRef {
    pub file: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Family a [`Diagnostic`] belongs to. Replaces the per-crate diagnostic
/// taxonomy; consumers discriminate on this instead of on the Rust type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    /// Source syntax or parser finding.
    Syntax,
    /// KIR/schema or semantic validation finding.
    Validation,
    /// Rulepack or legality finding.
    Legality,
    /// Runtime, datalog, or simulation execution issue.
    Execution,
    /// Capability or analysis readiness issue.
    Readiness,
    /// Mutation or transaction feasibility issue.
    Mutation,
    /// Source lint finding.
    Lint,
}

/// Canonical diagnostic shared across every Mercurio layer.
///
/// One type, discriminated by [`DiagnosticKind`], replaces the family of
/// per-crate `*Diagnostic` structs. `subjects` holds the element ids the
/// diagnostic is about (zero, one, or many); richer provenance belongs on the
/// Evidence noun rather than here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub kind: DiagnosticKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
    /// Source locations associated with the diagnostic, when known.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_spans: Vec<SourceSpanRef>,
}

impl Diagnostic {
    /// Construct a diagnostic of the given family and severity.
    pub fn new(
        kind: DiagnosticKind,
        severity: Severity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            kind,
            message: message.into(),
            subjects: Vec::new(),
            source_spans: Vec::new(),
        }
    }

    /// Construct a KIR validation diagnostic (`Validation` / `Error`),
    /// optionally about a single element.
    pub fn validation(
        code: impl Into<String>,
        message: impl Into<String>,
        subject: Option<String>,
    ) -> Self {
        let mut diagnostic = Self::new(DiagnosticKind::Validation, Severity::Error, code, message);
        diagnostic.subjects = subject.into_iter().collect();
        diagnostic
    }

    /// Add an element id this diagnostic is about.
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subjects.push(subject.into());
        self
    }

    /// Attach source locations to the diagnostic.
    pub fn with_source_spans(mut self, spans: impl IntoIterator<Item = SourceSpanRef>) -> Self {
        self.source_spans.extend(spans);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KirFieldKind {
    Scalar,
    Reference,
    ReferenceList,
    Expression,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KirFieldSpec {
    pub kind: KirFieldKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KirFieldRegistry {
    fields: BTreeMap<String, KirFieldSpec>,
}

impl KirFieldRegistry {
    pub fn structural() -> Self {
        let mut registry = Self {
            fields: BTreeMap::new(),
        };
        registry.register_fields(STRUCTURAL_FIELD_SPECS.iter().copied());
        registry
    }

    #[deprecated(note = "use structural(); domain fields must be registered by a language layer")]
    pub fn standard() -> Self {
        Self::structural()
    }

    pub fn from_document(document: &KirDocument) -> Self {
        let mut registry = Self::structural();
        registry.extend_from_document(document);
        registry
    }

    pub fn register_field(&mut self, name: impl Into<String>, kind: KirFieldKind) {
        self.fields.insert(name.into(), KirFieldSpec { kind });
    }

    pub fn register_fields<I, N>(&mut self, fields: I)
    where
        I: IntoIterator<Item = (N, KirFieldKind)>,
        N: Into<String>,
    {
        for (name, kind) in fields {
            self.register_field(name, kind);
        }
    }

    pub fn extend_from_document(&mut self, document: &KirDocument) {
        for element in &document.elements {
            let Some((name, spec)) = metamodel_feature_field_spec(element) else {
                continue;
            };
            self.fields.entry(name).or_insert(spec);
        }
    }

    pub fn field(&self, name: &str) -> Option<KirFieldSpec> {
        self.fields.get(name).copied()
    }

    pub fn reference_ids<'a>(&self, field: &str, value: &'a Value) -> Vec<&'a str> {
        match self.field(field).map(|spec| spec.kind) {
            Some(KirFieldKind::Reference) => reference_list_items(value),
            Some(KirFieldKind::ReferenceList) => reference_list_items(value),
            _ => Vec::new(),
        }
    }

    fn validate_value(
        &self,
        field: &str,
        value: &Value,
        element_id: &str,
        strict_shapes: bool,
        current_schema_shapes: bool,
    ) -> Option<Diagnostic> {
        let spec = self.field(field)?;
        let valid = match spec.kind {
            KirFieldKind::Scalar => {
                value.is_string() || value.is_number() || value.is_boolean() || value.is_null()
            }
            KirFieldKind::Reference if current_schema_shapes => {
                value.is_string() || value.is_null()
            }
            KirFieldKind::Reference if strict_shapes => {
                value.is_string()
                    || value
                        .as_array()
                        .is_some_and(|items| items.iter().all(Value::is_string))
                    || value.is_null()
            }
            KirFieldKind::Reference => {
                value.is_string()
                    || value
                        .as_array()
                        .is_some_and(|items| items.iter().all(Value::is_string))
                    || value.is_null()
            }
            KirFieldKind::ReferenceList if current_schema_shapes => {
                value
                    .as_array()
                    .is_some_and(|items| items.iter().all(Value::is_string))
                    || value.is_null()
            }
            KirFieldKind::ReferenceList if strict_shapes => {
                value.is_string()
                    || value
                        .as_array()
                        .is_some_and(|items| items.iter().all(Value::is_string))
                    || value.is_null()
            }
            KirFieldKind::ReferenceList => {
                value.is_string()
                    || value
                        .as_array()
                        .is_some_and(|items| items.iter().all(Value::is_string))
                    || value.is_null()
            }
            KirFieldKind::Expression | KirFieldKind::Metadata => {
                value.is_object() || value.is_array() || value.is_null()
            }
        };

        (!valid).then(|| {
            Diagnostic::validation(
                "kir.element.property.shape",
                format!(
                    "KIR element {element_id} property `{field}` has invalid shape for {:?}",
                    spec.kind
                ),
                Some(element_id.to_string()),
            )
        })
    }

    fn unknown_field_diagnostic(&self, field: &str, element_id: &str) -> Option<Diagnostic> {
        (self.field(field).is_none() && !field.starts_with("x_")).then(|| {
            Diagnostic::validation(
                "kir.element.property.unknown",
                format!(
                    "KIR element {element_id} property `{field}` is not registered in the field contract"
                ),
                Some(element_id.to_string()),
            )
        })
    }
}

pub const STRUCTURAL_FIELD_SPECS: &[(&str, KirFieldKind)] = &[
    ("declared_name", KirFieldKind::Scalar),
    ("declared_short_name", KirFieldKind::Scalar),
    ("name", KirFieldKind::Scalar),
    ("qualified_name", KirFieldKind::Scalar),
    ("short_name", KirFieldKind::Scalar),
    ("element_id", KirFieldKind::Scalar),
    ("kir_property", KirFieldKind::Scalar),
    ("feature_kind", KirFieldKind::Scalar),
    ("type_label", KirFieldKind::Scalar),
    ("metamodel_language", KirFieldKind::Scalar),
    ("metamodel_layer", KirFieldKind::Scalar),
    ("pilot_library_group", KirFieldKind::Scalar),
    ("source_file", KirFieldKind::Scalar),
    ("source_language", KirFieldKind::Scalar),
    ("is_abstract", KirFieldKind::Scalar),
    ("is_conjugated", KirFieldKind::Scalar),
    ("is_derived", KirFieldKind::Scalar),
    ("is_end", KirFieldKind::Scalar),
    ("is_variable", KirFieldKind::Scalar),
    ("is_readonly", KirFieldKind::Scalar),
    ("is_ordered", KirFieldKind::Scalar),
    ("is_unique", KirFieldKind::Scalar),
    ("is_library_element", KirFieldKind::Scalar),
    ("is_implied", KirFieldKind::Scalar),
    ("is_initial", KirFieldKind::Scalar),
    ("trigger", KirFieldKind::Scalar),
    ("trigger_kind", KirFieldKind::Scalar),
    ("source_is_initial", KirFieldKind::Scalar),
    ("lower", KirFieldKind::Scalar),
    ("upper", KirFieldKind::Scalar),
    ("multiplicity", KirFieldKind::Scalar),
    ("multiplicity_lower", KirFieldKind::Scalar),
    ("multiplicity_upper", KirFieldKind::Scalar),
    ("declared_multiplicity", KirFieldKind::Scalar),
    ("owner", KirFieldKind::Reference),
    ("owning_type", KirFieldKind::Reference),
    ("owning_definition", KirFieldKind::Reference),
    ("owning_namespace", KirFieldKind::Reference),
    ("definition", KirFieldKind::Reference),
    ("metatype", KirFieldKind::Reference),
    ("source_feature", KirFieldKind::Reference),
    ("parent_state", KirFieldKind::Reference),
    ("source", KirFieldKind::Reference),
    ("target", KirFieldKind::Reference),
    ("members", KirFieldKind::ReferenceList),
    ("features", KirFieldKind::ReferenceList),
    ("owned_element", KirFieldKind::ReferenceList),
    ("ownedElement", KirFieldKind::ReferenceList),
    ("owned_features", KirFieldKind::ReferenceList),
    ("owned_feature", KirFieldKind::ReferenceList),
    ("specializes", KirFieldKind::ReferenceList),
    ("subsets", KirFieldKind::ReferenceList),
    ("subsetted_features", KirFieldKind::ReferenceList),
    ("type", KirFieldKind::ReferenceList),
    ("featuring_type", KirFieldKind::ReferenceList),
    ("chaining_feature", KirFieldKind::ReferenceList),
    ("parts", KirFieldKind::ReferenceList),
    ("items", KirFieldKind::ReferenceList),
    ("expression", KirFieldKind::Scalar),
    ("expression_ir", KirFieldKind::Expression),
    ("metadata", KirFieldKind::Metadata),
    ("source_span", KirFieldKind::Metadata),
    ("doc", KirFieldKind::Metadata),
];

fn metamodel_feature_field_spec(element: &KirElement) -> Option<(String, KirFieldSpec)> {
    if element.kind != "MetamodelFeature" {
        return None;
    }
    let property = string_property(&element.properties, "kir_property")
        .or_else(|| string_property(&element.properties, "declared_name"))?;
    let property = property.trim();
    if property.is_empty() {
        return None;
    }

    Some((
        property.to_string(),
        KirFieldSpec {
            kind: metamodel_feature_field_kind(element),
        },
    ))
}

fn metamodel_feature_field_kind(element: &KirElement) -> KirFieldKind {
    let feature_kind = string_property(&element.properties, "feature_kind")
        .map(|value| value.to_ascii_lowercase());
    match feature_kind.as_deref() {
        Some("reference") | Some("relationship") | Some("endpoint") => {
            reference_kind_from_upper(element.properties.get("upper"))
        }
        Some("expression") | Some("expression_ir") => KirFieldKind::Expression,
        Some("metadata") | Some("annotation") | Some("documentation") => KirFieldKind::Metadata,
        Some(value) if value.contains("reference") => {
            reference_kind_from_upper(element.properties.get("upper"))
        }
        Some(value) if value.contains("expression") => KirFieldKind::Expression,
        Some(value) if value.contains("metadata") => KirFieldKind::Metadata,
        _ => KirFieldKind::Scalar,
    }
}

fn reference_kind_from_upper(upper: Option<&Value>) -> KirFieldKind {
    match multiplicity_upper_is_one(upper) {
        Some(true) => KirFieldKind::Reference,
        Some(false) => KirFieldKind::ReferenceList,
        None => KirFieldKind::ReferenceList,
    }
}

fn multiplicity_upper_is_one(upper: Option<&Value>) -> Option<bool> {
    match upper? {
        Value::Number(number) => number.as_u64().map(|value| value == 1),
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed == "1" {
                Some(true)
            } else {
                Some(false)
            }
        }
        Value::Null => None,
        _ => Some(false),
    }
}

fn string_property<'a>(properties: &'a BTreeMap<String, Value>, property: &str) -> Option<&'a str> {
    properties.get(property).and_then(Value::as_str)
}

fn reference_list_items(value: &Value) -> Vec<&str> {
    match value {
        Value::String(value) => vec![value.as_str()],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

impl fmt::Display for KirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read KIR document: {err}"),
            Self::Json(err) => write!(f, "failed to parse KIR document: {err}"),
            Self::DuplicateId(id) => write!(f, "duplicate KIR element id: {id}"),
            Self::Validation(diagnostics) => {
                write!(f, "invalid KIR document")?;
                if let Some(first) = diagnostics.first() {
                    write!(f, ": {}", first.message)?;
                }
                Ok(())
            }
            Self::Frontend(err) => write!(f, "{err}"),
            Self::Model(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for KirError {}

impl From<std::io::Error> for KirError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for KirError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl KirDocument {
    pub fn representative_example() -> Result<Self, KirError> {
        Self::from_str(REPRESENTATIVE_KIR_JSON)
    }

    pub fn from_str(input: &str) -> Result<Self, KirError> {
        let document: Self = serde_json::from_str(input)?;
        document.validate_persisted()?;
        Ok(document)
    }

    pub fn from_str_with_registered_fields<I, N>(input: &str, fields: I) -> Result<Self, KirError>
    where
        I: IntoIterator<Item = (N, KirFieldKind)>,
        N: Into<String>,
    {
        let document: Self = serde_json::from_str(input)?;
        document.validate_persisted_with_registered_fields(fields)?;
        Ok(document)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, KirError> {
        let input = std::str::from_utf8(bytes)
            .map_err(|_| KirError::Model("KIR bytes are not valid UTF-8".to_string()))?;
        Self::from_str(input)
    }

    pub fn from_path(path: &Path) -> Result<Self, KirError> {
        let input = std::fs::read_to_string(path)?;
        Self::from_str(&input)
    }

    pub fn from_path_with_registered_fields<I, N>(path: &Path, fields: I) -> Result<Self, KirError>
    where
        I: IntoIterator<Item = (N, KirFieldKind)>,
        N: Into<String>,
    {
        let input = std::fs::read_to_string(path)?;
        Self::from_str_with_registered_fields(&input, fields)
    }

    pub fn validate(&self) -> Result<(), KirError> {
        self.validate_with_options(
            ValidationOptions {
                require_schema_version: false,
                reject_unknown_fields: false,
                strict_field_shapes: false,
            },
            KirFieldRegistry::from_document(self),
        )
    }

    pub fn validate_strict_field_contract(&self) -> Result<(), KirError> {
        self.validate_with_options(
            ValidationOptions {
                require_schema_version: false,
                reject_unknown_fields: true,
                strict_field_shapes: true,
            },
            KirFieldRegistry::from_document(self),
        )
    }

    pub fn validate_persisted(&self) -> Result<(), KirError> {
        self.validate_with_options(
            ValidationOptions {
                require_schema_version: true,
                reject_unknown_fields: true,
                strict_field_shapes: true,
            },
            KirFieldRegistry::from_document(self),
        )
    }

    pub fn validate_persisted_with_registered_fields<I, N>(&self, fields: I) -> Result<(), KirError>
    where
        I: IntoIterator<Item = (N, KirFieldKind)>,
        N: Into<String>,
    {
        let mut field_registry = KirFieldRegistry::from_document(self);
        field_registry.register_fields(fields);
        self.validate_with_options(
            ValidationOptions {
                require_schema_version: true,
                reject_unknown_fields: true,
                strict_field_shapes: true,
            },
            field_registry,
        )
    }

    pub fn schema_version(&self) -> Option<&str> {
        self.metadata
            .get(KIR_SCHEMA_VERSION_METADATA_KEY)
            .and_then(Value::as_str)
    }

    pub fn with_schema_version(mut self) -> Self {
        self.set_schema_version();
        self
    }

    pub fn normalized_for_persistence(mut self) -> Self {
        self.set_schema_version();
        let field_registry = KirFieldRegistry::from_document(&self);
        for element in &mut self.elements {
            normalize_reference_shapes(&field_registry, element);
            if !element.properties.contains_key("qualified_name")
                && let Some(qualified_name) = qualified_name_from_element_id(&element.id)
            {
                element
                    .properties
                    .insert("qualified_name".to_string(), Value::String(qualified_name));
            }
        }
        self
    }

    pub fn normalized_for_persistence_with_registered_fields<I, N>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = (N, KirFieldKind)>,
        N: Into<String>,
    {
        self.set_schema_version();
        let mut field_registry = KirFieldRegistry::from_document(&self);
        field_registry.register_fields(fields);
        for element in &mut self.elements {
            normalize_reference_shapes(&field_registry, element);
            if !element.properties.contains_key("qualified_name")
                && let Some(qualified_name) = qualified_name_from_element_id(&element.id)
            {
                element
                    .properties
                    .insert("qualified_name".to_string(), Value::String(qualified_name));
            }
        }
        self
    }

    pub fn set_schema_version(&mut self) {
        self.metadata.insert(
            KIR_SCHEMA_VERSION_METADATA_KEY.to_string(),
            Value::String(KIR_SCHEMA_VERSION.to_string()),
        );
    }

    fn validate_with_options(
        &self,
        options: ValidationOptions,
        field_registry: KirFieldRegistry,
    ) -> Result<(), KirError> {
        let mut diagnostics = Vec::new();
        let mut seen = BTreeSet::new();
        let current_schema_shapes = options.strict_field_shapes;

        if options.require_schema_version {
            match self.metadata.get(KIR_SCHEMA_VERSION_METADATA_KEY) {
                Some(Value::String(version)) if SUPPORTED_KIR_SCHEMA_VERSIONS.contains(&version.as_str()) => {}
                Some(Value::String(version)) => diagnostics.push(Diagnostic::validation(
                    "kir.document.schema_version.unsupported",
                    format!(
                        "KIR document schema version `{version}` is not supported; expected one of {}",
                        SUPPORTED_KIR_SCHEMA_VERSIONS.join(", ")
                    ),
                    None,
                )),
                Some(_) => diagnostics.push(Diagnostic::validation(
                    "kir.document.schema_version.invalid",
                    format!(
                        "KIR document metadata `{KIR_SCHEMA_VERSION_METADATA_KEY}` must be a string"
                    ),
                    None,
                )),
                None => diagnostics.push(Diagnostic::validation(
                    "kir.document.schema_version.missing",
                    format!(
                        "KIR document metadata must include `{KIR_SCHEMA_VERSION_METADATA_KEY}`"
                    ),
                    None,
                )),
            }
        }

        for element in &self.elements {
            let trimmed_id = element.id.trim();
            if trimmed_id.is_empty() {
                diagnostics.push(Diagnostic::validation(
                    "kir.element.id.empty",
                    "KIR element id must not be empty".to_string(),
                    None,
                ));
            } else if trimmed_id != element.id {
                diagnostics.push(Diagnostic::validation(
                    "kir.element.id.invalid",
                    format!(
                        "KIR element id must not contain leading or trailing whitespace: {}",
                        element.id
                    ),
                    Some(element.id.clone()),
                ));
            }

            if element.kind.trim().is_empty() {
                diagnostics.push(Diagnostic::validation(
                    "kir.element.kind.empty",
                    format!("KIR element {} must declare a semantic kind", element.id),
                    Some(element.id.clone()),
                ));
            }

            if element.layer > 2 {
                diagnostics.push(Diagnostic::validation(
                    "kir.element.layer.unsupported",
                    format!(
                        "KIR element {} uses unsupported layer {}",
                        element.id, element.layer
                    ),
                    Some(element.id.clone()),
                ));
            }

            if !element.id.is_empty() && !seen.insert(element.id.clone()) {
                diagnostics.push(Diagnostic::validation(
                    "kir.element.id.duplicate",
                    format!("duplicate KIR element id: {}", element.id),
                    Some(element.id.clone()),
                ));
            }

            if options.strict_field_shapes
                && qualified_name_from_element_id(&element.id).is_some()
                && !element
                    .properties
                    .get("qualified_name")
                    .is_some_and(Value::is_string)
            {
                diagnostics.push(Diagnostic::validation(
                    "kir.element.qualified_name.missing",
                    format!(
                        "KIR element {} must include string property `qualified_name`",
                        element.id
                    ),
                    Some(element.id.clone()),
                ));
            }

            for (property, value) in &element.properties {
                if options.reject_unknown_fields {
                    if let Some(diagnostic) =
                        field_registry.unknown_field_diagnostic(property, &element.id)
                    {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                }
                if let Some(diagnostic) = field_registry.validate_value(
                    property,
                    value,
                    &element.id,
                    options.strict_field_shapes,
                    current_schema_shapes,
                ) {
                    diagnostics.push(diagnostic);
                }
                validate_structured_property(property, value, &element.id, &mut diagnostics);
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(KirError::Validation(diagnostics))
        }
    }

    pub fn merge<I>(documents: I) -> Result<Self, KirError>
    where
        I: IntoIterator<Item = KirDocument>,
    {
        Self::merge_with_validation(documents, None::<std::iter::Empty<(&str, KirFieldKind)>>)
    }

    pub fn merge_with_registered_fields<I, F, N>(documents: I, fields: F) -> Result<Self, KirError>
    where
        I: IntoIterator<Item = KirDocument>,
        F: IntoIterator<Item = (N, KirFieldKind)>,
        N: Into<String>,
    {
        Self::merge_with_validation(documents, Some(fields))
    }

    fn merge_with_validation<I, F, N>(documents: I, fields: Option<F>) -> Result<Self, KirError>
    where
        I: IntoIterator<Item = KirDocument>,
        F: IntoIterator<Item = (N, KirFieldKind)>,
        N: Into<String>,
    {
        let mut seen = BTreeSet::new();
        let mut elements = Vec::new();
        let mut source_metadata = Vec::new();

        for document in documents {
            if !document.metadata.is_empty() {
                source_metadata.push(Value::Object(document.metadata.into_iter().collect()));
            }
            for element in document.elements {
                if !seen.insert(element.id.clone()) {
                    return Err(KirError::DuplicateId(element.id));
                }
                elements.push(element);
            }
        }

        elements.sort_by(|left, right| left.id.cmp(&right.id));
        let mut metadata = BTreeMap::from([(
            KIR_SCHEMA_VERSION_METADATA_KEY.to_string(),
            Value::String(KIR_SCHEMA_VERSION.to_string()),
        )]);
        if !source_metadata.is_empty() {
            metadata.insert("merged_sources".to_string(), Value::Array(source_metadata));
        }

        let merged = Self { metadata, elements };
        let merged = if let Some(fields) = fields {
            let fields = fields
                .into_iter()
                .map(|(name, kind)| (name.into(), kind))
                .collect::<Vec<(String, KirFieldKind)>>();
            let normalized = merged.normalized_for_persistence_with_registered_fields(
                fields.iter().map(|(name, kind)| (name.as_str(), *kind)),
            );
            normalized.validate_persisted_with_registered_fields(
                fields.iter().map(|(name, kind)| (name.as_str(), *kind)),
            )?;
            normalized
        } else {
            let normalized = merged.normalized_for_persistence();
            normalized.validate_persisted()?;
            normalized
        };
        Ok(merged)
    }

    pub fn write_pretty_to_path(&self, path: &Path) -> Result<(), KirError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(
            path,
            serde_json::to_string_pretty(&self.clone().normalized_for_persistence())?,
        )?;
        Ok(())
    }
}

fn normalize_reference_shapes(field_registry: &KirFieldRegistry, element: &mut KirElement) {
    for (property, value) in &mut element.properties {
        match field_registry.field(property).map(|spec| spec.kind) {
            Some(KirFieldKind::Reference) => {
                let Some(items) = value.as_array() else {
                    continue;
                };

                *value = match items.as_slice() {
                    [] => Value::Null,
                    [single] if single.is_string() => single.clone(),
                    _ => continue,
                };
            }
            Some(KirFieldKind::ReferenceList) => {
                if value.is_string() {
                    *value = Value::Array(vec![value.clone()]);
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ValidationOptions {
    require_schema_version: bool,
    reject_unknown_fields: bool,
    strict_field_shapes: bool,
}

fn validate_structured_property(
    property: &str,
    value: &Value,
    element_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match property {
        "metadata" => validate_metadata(value, element_id, diagnostics),
        "source_span" => validate_source_span(value, element_id, "source_span", diagnostics),
        "expression_ir" => validate_expression_ir(value, element_id, "expression_ir", diagnostics),
        _ => {}
    }
}

fn validate_metadata(value: &Value, element_id: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(metadata) = value.as_object() else {
        return;
    };

    if let Some(source_file) = metadata.get("source_file")
        && !(source_file.is_string() || source_file.is_null())
    {
        diagnostics.push(Diagnostic::validation(
            "kir.element.metadata.source_file.shape",
            format!("KIR element {element_id} metadata `source_file` must be a string"),
            Some(element_id.to_string()),
        ));
    }

    if let Some(source_language) = metadata.get("source_language")
        && !(source_language.is_string() || source_language.is_null())
    {
        diagnostics.push(Diagnostic::validation(
            "kir.element.metadata.source_language.shape",
            format!("KIR element {element_id} metadata `source_language` must be a string"),
            Some(element_id.to_string()),
        ));
    }

    if let Some(generated) = metadata.get("generated")
        && !(generated.is_boolean() || generated.is_null())
    {
        diagnostics.push(Diagnostic::validation(
            "kir.element.metadata.generated.shape",
            format!("KIR element {element_id} metadata `generated` must be a boolean"),
            Some(element_id.to_string()),
        ));
    }

    if let Some(source_span) = metadata.get("source_span") {
        validate_source_span(source_span, element_id, "metadata.source_span", diagnostics);
    }
}

fn validate_source_span(
    value: &Value,
    element_id: &str,
    field: &'static str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        return;
    }
    let Some(span) = value.as_object() else {
        diagnostics.push(Diagnostic::validation(
            "kir.element.source_span.shape",
            format!("KIR element {element_id} `{field}` must be an object"),
            Some(element_id.to_string()),
        ));
        return;
    };

    for key in ["start_line", "end_line"] {
        if !span.get(key).is_some_and(Value::is_number) {
            diagnostics.push(Diagnostic::validation(
                "kir.element.source_span.shape",
                format!("KIR element {element_id} `{field}.{key}` must be a number"),
                Some(element_id.to_string()),
            ));
        }
    }
    for key in ["start_col", "end_col"] {
        if let Some(value) = span.get(key)
            && !value.is_number()
        {
            diagnostics.push(Diagnostic::validation(
                "kir.element.source_span.shape",
                format!("KIR element {element_id} `{field}.{key}` must be a number when present"),
                Some(element_id.to_string()),
            ));
        }
    }
}

fn validate_expression_ir(
    value: &Value,
    element_id: &str,
    field: &'static str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        return;
    }
    if !value.is_object() {
        diagnostics.push(Diagnostic::validation(
            "kir.element.expression_ir.shape",
            format!("KIR element {element_id} `{field}` must be an object"),
            Some(element_id.to_string()),
        ));
        return;
    }

    if !value.get("kind").is_some_and(Value::is_string) {
        diagnostics.push(Diagnostic::validation(
            "kir.element.expression_ir.kind.missing",
            format!("KIR element {element_id} `{field}.kind` must be a string"),
            Some(element_id.to_string()),
        ));
        return;
    }

    if let Err(err) = ExpressionIr::from_value(value) {
        diagnostics.push(Diagnostic::validation(
            "kir.element.expression_ir.shape",
            format!("KIR element {element_id} `{field}` is invalid: {err}"),
            Some(element_id.to_string()),
        ));
    }
}

fn qualified_name_from_element_id(id: &str) -> Option<String> {
    const PREFIXES: [&str; 14] = [
        "pkg.",
        "type.",
        "feature.",
        "part.",
        "item.",
        "requirement.",
        "case.",
        "action.",
        "state.",
        "transition.",
        "connection.",
        "metadata.",
        "relationship.",
        "doc.",
    ];

    PREFIXES
        .iter()
        .find_map(|prefix| id.strip_prefix(prefix))
        .map(str::to_string)
}

pub fn inferred_layer(id: &str, kind: &str, properties: &BTreeMap<String, Value>) -> u8 {
    if properties
        .get("is_library_element")
        .is_some_and(|value| value.as_bool() == Some(true))
    {
        return 1;
    }
    if let Some(Value::String(group)) = properties.get("pilot_library_group") {
        return match group.as_str() {
            "Kernel Libraries" | "KerML Kernel" | "Kernel" => 0,
            _ => 1,
        };
    }
    if let Some(Value::String(layer)) = properties.get("metamodel_layer") {
        return match layer.as_str() {
            "kernel" | "core" | "kerml" => 0,
            "systems" | "domain" | "stdlib" | "library" => 1,
            "model" | "user" => 2,
            _ => 2,
        };
    }

    if kind.starts_with("Core::")
        || kind.starts_with("KerML::Kernel::")
        || kind.starts_with("Kernel::")
    {
        return 0;
    }
    if kind.starts_with("SysML::Libraries::")
        || kind.starts_with("Model::Libraries::")
        || kind.starts_with("Library::")
    {
        return 1;
    }
    if id.starts_with("Core::")
        || id.starts_with("KerML::")
        || id.starts_with("ScalarValues::")
        || id.starts_with("Base::")
    {
        return 1;
    }

    2
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use super::{KirDocument, KirElement, KirError, KirFieldKind, KirFieldRegistry};

    #[test]
    fn merge_rejects_duplicate_ids() {
        let left = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "dup".to_string(),
                kind: "core.Type".to_string(),
                layer: 0,
                properties: Default::default(),
            }],
        };
        let right = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "dup".to_string(),
                kind: "model.PartDefinition".to_string(),
                layer: 1,
                properties: Default::default(),
            }],
        };

        let error = KirDocument::merge([left, right]).unwrap_err();
        assert!(matches!(error, KirError::DuplicateId(id) if id == "dup"));
    }

    #[test]
    fn validate_rejects_empty_element_id() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: String::new(),
                kind: "Core::Core::Type".to_string(),
                layer: 0,
                properties: Default::default(),
            }],
        };

        let error = document.validate().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.id.empty")
        );
    }

    #[test]
    fn validate_rejects_unsupported_layer() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Bad.Layer".to_string(),
                kind: "Core::Core::Type".to_string(),
                layer: 3,
                properties: Default::default(),
            }],
        };

        let error = document.validate().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.layer.unsupported")
        );
    }

    #[test]
    fn kir_element_serialization_omits_layer() {
        let element = KirElement {
            id: "type.Demo.Vehicle".to_string(),
            kind: "PartDefinition".to_string(),
            layer: 2,
            properties: Default::default(),
        };

        let encoded = serde_json::to_value(element).unwrap();

        assert!(encoded.get("layer").is_none());
        assert_eq!(encoded["kind"], "PartDefinition");
    }

    #[test]
    fn kir_element_deserialization_accepts_legacy_layer() {
        let element: KirElement = serde_json::from_value(serde_json::json!({
            "id": "type.Demo.Vehicle",
            "kind": "PartDefinition",
            "layer": 2
        }))
        .unwrap();

        assert_eq!(element.layer, 2);
    }

    #[test]
    fn kir_element_deserialization_infers_layer_when_missing() {
        let kernel: KirElement = serde_json::from_value(serde_json::json!({
            "id": "type.Kernel.Element",
            "kind": "Core::Core::Type"
        }))
        .unwrap();
        let model: KirElement = serde_json::from_value(serde_json::json!({
            "id": "type.Demo.Vehicle",
            "kind": "PartDefinition"
        }))
        .unwrap();

        assert_eq!(kernel.layer, 0);
        assert_eq!(model.layer, 2);
    }

    #[test]
    fn validate_rejects_registered_reference_field_with_wrong_shape() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "feature.Demo.Vehicle.engine".to_string(),
                kind: "Model::PartUsage".to_string(),
                layer: 2,
                properties: [("owner".to_string(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
            }],
        };

        let error = document.validate().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.property.shape")
        );
    }

    #[test]
    fn validate_accepts_registered_reference_list_as_scalar_or_array() {
        let scalar = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [(
                    "specializes".to_string(),
                    Value::String("Parts::Part".to_string()),
                )]
                .into_iter()
                .collect(),
            }],
        };
        let array = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [(
                    "specializes".to_string(),
                    Value::Array(vec![Value::String("Parts::Part".to_string())]),
                )]
                .into_iter()
                .collect(),
            }],
        };

        scalar.validate().unwrap();
        array.validate().unwrap();
    }

    #[test]
    fn current_schema_validation_requires_reference_lists_to_be_arrays() {
        let document = KirDocument {
            metadata: [(
                super::KIR_SCHEMA_VERSION_METADATA_KEY.to_string(),
                json!(super::KIR_SCHEMA_VERSION),
            )]
            .into_iter()
            .collect(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [
                    (
                        "qualified_name".to_string(),
                        Value::String("Demo.Vehicle".to_string()),
                    ),
                    (
                        "specializes".to_string(),
                        Value::String("type.Demo.BaseVehicle".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        };

        let error = document.validate_persisted().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.property.shape")
        );
    }

    #[test]
    fn current_schema_validation_requires_single_references_to_be_scalars() {
        let document = KirDocument {
            metadata: [(
                super::KIR_SCHEMA_VERSION_METADATA_KEY.to_string(),
                json!(super::KIR_SCHEMA_VERSION),
            )]
            .into_iter()
            .collect(),
            elements: vec![KirElement {
                id: "feature.Demo.Vehicle.engine".to_string(),
                kind: "Model::Parts::PartUsage".to_string(),
                layer: 2,
                properties: [
                    (
                        "qualified_name".to_string(),
                        Value::String("Demo.Vehicle.engine".to_string()),
                    ),
                    (
                        "owner".to_string(),
                        Value::Array(vec![Value::String("type.Demo.Vehicle".to_string())]),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        };

        let error = document.validate_persisted().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.property.shape")
        );
    }

    #[test]
    fn persisted_validation_rejects_legacy_schema_versions() {
        let document = KirDocument {
            metadata: [(
                super::KIR_SCHEMA_VERSION_METADATA_KEY.to_string(),
                json!("0.3"),
            )]
            .into_iter()
            .collect(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [
                    (
                        "qualified_name".to_string(),
                        Value::String("Demo.Vehicle".to_string()),
                    ),
                    (
                        "specializes".to_string(),
                        Value::String("type.Demo.BaseVehicle".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        };

        let error = document.validate_persisted().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.document.schema_version.unsupported")
        );
    }

    #[test]
    fn field_registry_derives_unknown_fields_from_metamodel_features() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![metamodel_feature(
                "metafeature.Demo.Thing.related_requirement",
                "Demo::Thing",
                "related_requirement",
                "reference",
                Some(json!(1)),
            )],
        };

        let registry = KirFieldRegistry::from_document(&document);

        assert_eq!(
            registry.field("related_requirement").map(|spec| spec.kind),
            Some(KirFieldKind::Reference)
        );
    }

    #[test]
    fn strict_field_contract_accepts_metamodel_declared_scalar_fields() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![
                metamodel_feature(
                    "metafeature.Demo.Thing.risk_score",
                    "Demo::Thing",
                    "risk_score",
                    "attribute",
                    None,
                ),
                KirElement {
                    id: "type.Demo.Sample".to_string(),
                    kind: "Demo::Thing".to_string(),
                    layer: 2,
                    properties: [
                        (
                            "qualified_name".to_string(),
                            Value::String("Demo.Sample".to_string()),
                        ),
                        ("risk_score".to_string(), json!(7)),
                    ]
                    .into_iter()
                    .collect(),
                },
            ],
        };

        document.validate_strict_field_contract().unwrap();
    }

    #[test]
    fn persistence_normalization_uses_metamodel_reference_list_fields() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![
                metamodel_feature(
                    "metafeature.Demo.Thing.related_requirements",
                    "Demo::Thing",
                    "related_requirements",
                    "reference",
                    Some(json!("*")),
                ),
                KirElement {
                    id: "type.Demo.Sample".to_string(),
                    kind: "Demo::Thing".to_string(),
                    layer: 2,
                    properties: [
                        (
                            "qualified_name".to_string(),
                            Value::String("Demo.Sample".to_string()),
                        ),
                        (
                            "related_requirements".to_string(),
                            Value::String("requirement.Demo.R1".to_string()),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                },
            ],
        }
        .normalized_for_persistence();

        assert_eq!(
            document.elements[1].properties.get("related_requirements"),
            Some(&json!(["requirement.Demo.R1"]))
        );
        document.validate_persisted().unwrap();
    }

    #[test]
    fn persistence_normalization_wraps_reference_list_scalars() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [(
                    "specializes".to_string(),
                    Value::String("type.Demo.BaseVehicle".to_string()),
                )]
                .into_iter()
                .collect(),
            }],
        }
        .normalized_for_persistence();

        assert_eq!(
            document.elements[0].properties.get("specializes"),
            Some(&json!(["type.Demo.BaseVehicle"]))
        );
        document.validate_persisted().unwrap();
    }

    #[test]
    fn persisted_validation_uses_shared_expression_ir_contract() {
        let document = KirDocument {
            metadata: [(
                super::KIR_SCHEMA_VERSION_METADATA_KEY.to_string(),
                json!(super::KIR_SCHEMA_VERSION),
            )]
            .into_iter()
            .collect(),
            elements: vec![KirElement {
                id: "feature.Demo.Vehicle.mass".to_string(),
                kind: "Model::Attributes::AttributeUsage".to_string(),
                layer: 2,
                properties: [
                    (
                        "qualified_name".to_string(),
                        Value::String("Demo.Vehicle.mass".to_string()),
                    ),
                    (
                        "expression_ir".to_string(),
                        json!({
                            "kind": "binary",
                            "op": "add",
                            "left": {"kind": "literal", "value": 1}
                        }),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        };

        let error = document.validate_persisted().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.expression_ir.shape")
        );
    }

    #[test]
    fn strict_field_contract_rejects_unknown_non_extension_property() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [
                    (
                        "qualified_name".to_string(),
                        Value::String("Demo.Vehicle".to_string()),
                    ),
                    ("custom".to_string(), Value::String("value".to_string())),
                    (
                        "x_vendor".to_string(),
                        Value::String("preserved".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        };

        document.validate().unwrap();
        let error = document.validate_strict_field_contract().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.property.unknown")
        );
    }

    #[test]
    fn strict_field_contract_rejects_unregistered_domain_trace_fields() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "part.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartUsage".to_string(),
                layer: 2,
                properties: [
                    (
                        "qualified_name".to_string(),
                        Value::String("Demo.Vehicle".to_string()),
                    ),
                    ("satisfy".to_string(), json!(["requirement.Demo.Safe"])),
                ]
                .into_iter()
                .collect(),
            }],
        };

        let error = document.validate_strict_field_contract().unwrap_err();

        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.property.unknown")
        );
    }

    #[test]
    fn persisted_validation_requires_schema_version() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: Default::default(),
            }],
        };

        document.validate().unwrap();
        let error = document.validate_persisted().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.document.schema_version.missing")
        );
    }

    #[test]
    fn with_schema_version_normalizes_persisted_envelope() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: Default::default(),
            }],
        }
        .normalized_for_persistence();

        assert_eq!(document.schema_version(), Some(super::KIR_SCHEMA_VERSION));
        document.validate_persisted().unwrap();
    }

    #[test]
    fn persisted_validation_accepts_transition_initial_source_marker() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "transition.Demo.Machine.idle_go".to_string(),
                kind: "SysML::States::TransitionUsage".to_string(),
                layer: 2,
                properties: [
                    (
                        "qualified_name".to_string(),
                        Value::String("Demo.Machine.idle_go".to_string()),
                    ),
                    (
                        "source".to_string(),
                        Value::String("state.Demo.Machine.Idle".to_string()),
                    ),
                    (
                        "target".to_string(),
                        Value::String("state.Demo.Machine.Go".to_string()),
                    ),
                    ("trigger".to_string(), Value::String("start".to_string())),
                    (
                        "trigger_kind".to_string(),
                        Value::String("event".to_string()),
                    ),
                    (
                        "source_is_initial".to_string(),
                        Value::String("true".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        }
        .normalized_for_persistence();

        document
            .validate_persisted_with_registered_fields([
                ("trigger", KirFieldKind::Scalar),
                ("trigger_kind", KirFieldKind::Scalar),
                ("source_is_initial", KirFieldKind::Scalar),
            ])
            .unwrap();
    }

    #[test]
    fn persisted_validation_accepts_state_do_behavior() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "state.Demo.Bed.lifecycle.Heating".to_string(),
                kind: "SysML::Systems::StateUsage".to_string(),
                layer: 2,
                properties: [
                    (
                        "qualified_name".to_string(),
                        Value::String("Demo.Bed.lifecycle.Heating".to_string()),
                    ),
                    (
                        "do_behavior".to_string(),
                        serde_json::json!({
                            "kind": "rate_integration",
                            "rates": [
                                {
                                    "feature": "temperature",
                                    "rate": 2.2
                                }
                            ]
                        }),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        }
        .normalized_for_persistence();

        document
            .validate_persisted_with_registered_fields([("do_behavior", KirFieldKind::Metadata)])
            .unwrap();
    }

    #[test]
    fn representative_example_is_valid_persisted_kir() {
        let document = KirDocument::representative_example().unwrap();

        assert!(
            document
                .schema_version()
                .is_some_and(|version| super::SUPPORTED_KIR_SCHEMA_VERSIONS.contains(&version))
        );
        assert!(
            document
                .elements
                .iter()
                .any(|element| element.id == "pkg.Example")
        );
        assert!(
            document
                .elements
                .iter()
                .any(|element| element.id == "activity.Example.Startup")
        );
        document.validate_persisted().unwrap();
    }

    #[test]
    fn from_str_rejects_unknown_fields_in_persisted_kir() {
        let input = format!(
            r#"{{
  "metadata": {{ "kir_schema_version": "{}" }},
  "elements": [
    {{
      "id": "type.Demo.Vehicle",
      "kind": "Model::Systems::PartDefinition",
      "layer": 2,
      "properties": {{ "qualified_name": "Demo.Vehicle", "custom": "value" }}
    }}
  ]
}}"#,
            super::KIR_SCHEMA_VERSION
        );

        let error = KirDocument::from_str(&input).unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.property.unknown")
        );
    }

    #[test]
    fn merge_preserves_document_metadata_as_sources() {
        let left = KirDocument {
            metadata: [("source".to_string(), Value::String("stdlib".to_string()))]
                .into_iter()
                .collect(),
            elements: vec![],
        };
        let right = KirDocument {
            metadata: [("source".to_string(), Value::String("user".to_string()))]
                .into_iter()
                .collect(),
            elements: vec![],
        };

        let merged = KirDocument::merge([left, right]).unwrap();
        assert_eq!(
            merged.metadata[super::KIR_SCHEMA_VERSION_METADATA_KEY],
            Value::String(super::KIR_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            merged.metadata["merged_sources"].as_array().unwrap().len(),
            2
        );
    }

    fn metamodel_feature(
        id: &str,
        owner: &str,
        kir_property: &str,
        feature_kind: &str,
        upper: Option<Value>,
    ) -> KirElement {
        let mut properties = [
            ("qualified_name".to_string(), Value::String(id.to_string())),
            ("owner".to_string(), Value::String(owner.to_string())),
            (
                "kir_property".to_string(),
                Value::String(kir_property.to_string()),
            ),
            (
                "feature_kind".to_string(),
                Value::String(feature_kind.to_string()),
            ),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        if let Some(upper) = upper {
            properties.insert("upper".to_string(), upper);
        }
        KirElement {
            id: id.to_string(),
            kind: "MetamodelFeature".to_string(),
            layer: 1,
            properties,
        }
    }
}
