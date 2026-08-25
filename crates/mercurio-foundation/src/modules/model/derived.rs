use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::graph::{Element, ElementProperties, Graph};

#[derive(Debug, Clone, Default)]
pub struct DerivedFeatureCache {
    revision: String,
    values: RefCell<BTreeMap<(String, String), Option<DerivedPropertyValue>>>,
}

#[derive(Debug, Clone, Default)]
pub struct DerivedFeatureRegistry {
    specs: Vec<DerivedFeatureSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedFeatureManifest {
    #[serde(default)]
    pub metamodel: Option<String>,
    #[serde(default)]
    pub derived_features: Vec<DerivedFeatureSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DerivedFeatureRule {
    Alias {
        source: String,
    },
    Subset {
        source: String,
        #[serde(default)]
        target_kind: Option<String>,
        #[serde(default)]
        target_type: Option<String>,
    },
    SubsetChain {
        source: String,
        target_feature: String,
        #[serde(default)]
        target_kind: Option<String>,
        #[serde(default)]
        target_type: Option<String>,
    },
    IntersectionSubsetChain {
        sources: Vec<String>,
        target_feature: String,
        #[serde(default)]
        target_kind: Option<String>,
        #[serde(default)]
        target_type: Option<String>,
    },
    Inverse {
        source: String,
    },
    Name,
    ShortName,
    QualifiedName,
    LibraryBoolean,
    Native {
        function: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedFeatureSpec {
    pub owner: String,
    pub feature: String,
    #[serde(flatten)]
    pub rule: DerivedFeatureRule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedPropertyValue {
    pub value: Value,
    pub source: DerivedPropertySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedPropertySource {
    ExplicitProperty,
    InverseRelation,
    ForwardRelation,
    DeclaredName,
    DeclaredShortName,
    Layer,
    OwnerNameChain,
    OwnedElementSubset,
    DocumentationOwner,
    ManifestAlias,
    ManifestSubset,
    ManifestSubsetChain,
    ManifestInverse,
    ManifestName,
    ManifestLibraryBoolean,
    ManifestNative,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedFeatureManifestError {
    InvalidManifest(String),
    UnknownNativeFunction(String),
}

impl fmt::Display for DerivedFeatureManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifest(message) => {
                write!(formatter, "invalid derived feature manifest: {message}")
            }
            Self::UnknownNativeFunction(function) => {
                write!(
                    formatter,
                    "unknown native derived feature function: {function}"
                )
            }
        }
    }
}

impl std::error::Error for DerivedFeatureManifestError {}

impl DerivedFeatureCache {
    pub fn new(revision: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
            values: RefCell::new(BTreeMap::new()),
        }
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn clear_for_revision(&mut self, revision: impl Into<String>) {
        self.revision = revision.into();
        self.values.get_mut().clear();
    }

    pub fn derived_property(
        &self,
        registry: &DerivedFeatureRegistry,
        graph: &Graph,
        element: &Element,
        feature: &str,
    ) -> Option<DerivedPropertyValue> {
        let key = (element.element_id.clone(), feature.to_string());
        if let Some(cached) = self.values.borrow().get(&key) {
            return cached.clone();
        }

        let value = registry.derived_property_uncached(graph, element, feature);
        self.values.borrow_mut().insert(key, value.clone());
        value
    }
}

impl DerivedFeatureRegistry {
    pub fn new(specs: Vec<DerivedFeatureSpec>) -> Self {
        Self { specs }
    }

    pub fn from_manifest(manifest: DerivedFeatureManifest) -> Self {
        Self::new(manifest.derived_features)
    }

    pub fn with_builtin_core_specs() -> Self {
        Self::new(builtin_core_specs())
    }

    pub fn with_manifest_and_builtins(
        manifest: Option<DerivedFeatureManifest>,
    ) -> Result<Self, DerivedFeatureManifestError> {
        let mut specs = manifest
            .map(|manifest| manifest.derived_features)
            .unwrap_or_default();
        specs.extend(builtin_core_specs());
        let registry = Self::new(specs);
        registry.validate()?;
        Ok(registry)
    }

    pub fn specs(&self) -> &[DerivedFeatureSpec] {
        &self.specs
    }

    pub fn validate(&self) -> Result<(), DerivedFeatureManifestError> {
        for spec in &self.specs {
            if let DerivedFeatureRule::Native { function } = &spec.rule
                && !is_native_function(function)
            {
                return Err(DerivedFeatureManifestError::UnknownNativeFunction(
                    function.clone(),
                ));
            }
        }
        Ok(())
    }

    fn derived_property_uncached(
        &self,
        graph: &Graph,
        element: &Element,
        feature: &str,
    ) -> Option<DerivedPropertyValue> {
        if let Some(value) = element.properties.get(feature) {
            return Some(DerivedPropertyValue {
                value: value.clone(),
                source: DerivedPropertySource::ExplicitProperty,
            });
        }

        if let Some(value) = self
            .matching_specs(element, feature)
            .find_map(|spec| derive_from_spec(self, graph, element, spec))
        {
            return Some(value);
        }

        derive_property_uncached_fallback(graph, element, feature)
    }

    fn matching_specs<'a>(
        &'a self,
        element: &'a Element,
        feature: &'a str,
    ) -> impl Iterator<Item = &'a DerivedFeatureSpec> {
        self.specs
            .iter()
            .filter(move |spec| spec.feature == feature && spec_matches_element(spec, element))
    }
}

pub fn builtin_core_derived_feature_manifest(metamodel: Option<String>) -> DerivedFeatureManifest {
    DerivedFeatureManifest {
        metamodel,
        derived_features: builtin_core_specs(),
    }
}

pub fn derived_property(
    graph: &Graph,
    cache: &DerivedFeatureCache,
    element: &Element,
    feature: &str,
) -> Option<DerivedPropertyValue> {
    let registry = DerivedFeatureRegistry::with_builtin_core_specs();
    cache.derived_property(&registry, graph, element, feature)
}

pub fn derived_properties(
    graph: &Graph,
    element: &Element,
) -> BTreeMap<String, DerivedPropertyValue> {
    let cache = DerivedFeatureCache::new("adhoc");
    let registry = DerivedFeatureRegistry::with_builtin_core_specs();
    let mut properties = BTreeMap::new();

    for feature in [
        "owner",
        "ownedElement",
        "documentation",
        "documentedElement",
        "annotatedElement",
        "name",
        "shortName",
        "qualifiedName",
        "isLibraryElement",
    ] {
        if let Some(value) = cache.derived_property(&registry, graph, element, feature) {
            properties.insert(feature.to_string(), value);
        }
    }

    properties
}

fn derive_property_uncached_fallback(
    graph: &Graph,
    element: &Element,
    feature: &str,
) -> Option<DerivedPropertyValue> {
    match feature {
        "owner" => derived_owner(graph, element),
        "ownedElement" => derived_owned_element(graph, element),
        "documentation" => derived_documentation(graph, element),
        "documentedElement" if is_documentation_element(element) => derived_owner(graph, element)
            .map(|mut value| {
                value.source = DerivedPropertySource::DocumentationOwner;
                value
            }),
        "annotatedElement" if is_documentation_element(element) => derived_owner(graph, element)
            .map(|mut value| {
                value.source = DerivedPropertySource::DocumentationOwner;
                value
            }),
        "name" => derived_name(element),
        "shortName" => derived_short_name(element),
        "qualifiedName" => derived_qualified_name(graph, element),
        "isLibraryElement" => Some(DerivedPropertyValue {
            value: Value::Bool(element.layer < 2),
            source: DerivedPropertySource::Layer,
        }),
        _ => None,
    }
}

fn derive_from_spec(
    registry: &DerivedFeatureRegistry,
    graph: &Graph,
    element: &Element,
    spec: &DerivedFeatureSpec,
) -> Option<DerivedPropertyValue> {
    match &spec.rule {
        DerivedFeatureRule::Alias { source } => {
            let value = if source == "__builtin_owner" {
                derived_owner(graph, element)
            } else {
                registry.derived_property_uncached(graph, element, source)
            };
            value.map(|mut value| {
                value.source = DerivedPropertySource::ManifestAlias;
                value
            })
        }
        DerivedFeatureRule::Subset {
            source,
            target_kind,
            target_type,
        } => {
            let source_value = registry.derived_property_uncached(graph, element, source)?;
            let ids = refs_from_value(&source_value.value)
                .into_iter()
                .filter(|id| {
                    graph.element_by_element_id(id).is_some_and(|candidate| {
                        target_matches(candidate, target_kind, target_type)
                    })
                })
                .collect::<Vec<_>>();
            (!ids.is_empty()).then_some(DerivedPropertyValue {
                value: value_for_refs(ids),
                source: DerivedPropertySource::ManifestSubset,
            })
        }
        DerivedFeatureRule::SubsetChain {
            source,
            target_feature,
            target_kind,
            target_type,
        } => {
            let source_value =
                registry.derived_property_uncached(graph, element, feature_name(source))?;
            let ids = refs_from_value(&source_value.value)
                .into_iter()
                .filter(|id| {
                    graph.element_by_element_id(id).is_some_and(|candidate| {
                        element_specializes(graph, candidate, target_feature)
                            && target_matches(candidate, target_kind, target_type)
                    })
                })
                .collect::<Vec<_>>();
            (!ids.is_empty()).then_some(DerivedPropertyValue {
                value: value_for_refs(ids),
                source: DerivedPropertySource::ManifestSubsetChain,
            })
        }
        DerivedFeatureRule::IntersectionSubsetChain {
            sources,
            target_feature,
            target_kind,
            target_type,
        } => {
            let ids = intersect_source_refs(registry, graph, element, sources)?
                .into_iter()
                .filter(|id| {
                    graph.element_by_element_id(id).is_some_and(|candidate| {
                        element_specializes(graph, candidate, target_feature)
                            && target_matches(candidate, target_kind, target_type)
                    })
                })
                .collect::<Vec<_>>();
            (!ids.is_empty()).then_some(DerivedPropertyValue {
                value: value_for_refs(ids),
                source: DerivedPropertySource::ManifestSubsetChain,
            })
        }
        DerivedFeatureRule::Inverse { source } => {
            if source == "owner" {
                return derived_owned_element(graph, element).map(|mut value| {
                    value.source = DerivedPropertySource::ManifestInverse;
                    value
                });
            }
            let ids = relation_targets(graph, element, source);
            (!ids.is_empty()).then_some(DerivedPropertyValue {
                value: Value::Array(ids.into_iter().map(Value::String).collect()),
                source: DerivedPropertySource::ManifestInverse,
            })
        }
        DerivedFeatureRule::Name => derived_name(element).map(|mut value| {
            value.source = DerivedPropertySource::ManifestName;
            value
        }),
        DerivedFeatureRule::ShortName => derived_short_name(element).map(|mut value| {
            value.source = DerivedPropertySource::ManifestName;
            value
        }),
        DerivedFeatureRule::QualifiedName => {
            derived_qualified_name(graph, element).map(|mut value| {
                value.source = DerivedPropertySource::ManifestName;
                value
            })
        }
        DerivedFeatureRule::LibraryBoolean => Some(DerivedPropertyValue {
            value: Value::Bool(element.layer < 2),
            source: DerivedPropertySource::ManifestLibraryBoolean,
        }),
        DerivedFeatureRule::Native { function } => derive_native_function(function, graph, element)
            .map(|mut value| {
                value.source = DerivedPropertySource::ManifestNative;
                value
            }),
    }
}

pub fn manifest_from_metadata(
    metadata: &BTreeMap<String, Value>,
) -> Result<Option<DerivedFeatureManifest>, DerivedFeatureManifestError> {
    let mut manifests = Vec::new();
    if let Some(value) = metadata.get("derived_feature_manifest").cloned() {
        manifests.push(parse_manifest_value(value)?);
    }
    if let Some(sources) = metadata.get("merged_sources").and_then(Value::as_array) {
        for source in sources {
            let Some(source_metadata) = source.as_object() else {
                continue;
            };
            if let Some(value) = source_metadata.get("derived_feature_manifest").cloned() {
                manifests.push(parse_manifest_value(value)?);
            }
        }
    }

    let mut derived_features = Vec::new();
    let mut metamodel = None;
    for manifest in manifests {
        metamodel = metamodel.or(manifest.metamodel);
        derived_features.extend(manifest.derived_features);
    }
    if derived_features.is_empty() {
        return Ok(None);
    }
    let manifest = DerivedFeatureManifest {
        metamodel,
        derived_features,
    };
    DerivedFeatureRegistry::from_manifest(manifest.clone()).validate()?;
    Ok(Some(manifest))
}

fn parse_manifest_value(
    value: Value,
) -> Result<DerivedFeatureManifest, DerivedFeatureManifestError> {
    serde_json::from_value(value)
        .map_err(|error| DerivedFeatureManifestError::InvalidManifest(error.to_string()))
}

fn builtin_core_specs() -> Vec<DerivedFeatureSpec> {
    vec![
        DerivedFeatureSpec {
            owner: "*".to_string(),
            feature: "owner".to_string(),
            rule: DerivedFeatureRule::Alias {
                source: "__builtin_owner".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "*".to_string(),
            feature: "ownedElement".to_string(),
            rule: DerivedFeatureRule::Inverse {
                source: "owner".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Element".to_string(),
            feature: "documentation".to_string(),
            rule: DerivedFeatureRule::Subset {
                source: "ownedElement".to_string(),
                target_kind: Some("Core::Root::Documentation".to_string()),
                target_type: None,
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Documentation".to_string(),
            feature: "documentedElement".to_string(),
            rule: DerivedFeatureRule::Alias {
                source: "owner".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Documentation".to_string(),
            feature: "annotatedElement".to_string(),
            rule: DerivedFeatureRule::Alias {
                source: "owner".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Element".to_string(),
            feature: "owningNamespace".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.owning_namespace".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Namespace".to_string(),
            feature: "member".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.namespace_member".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Namespace".to_string(),
            feature: "membership".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.namespace_membership".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Relationship".to_string(),
            feature: "relatedElement".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.relationship_related_element".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Import".to_string(),
            feature: "importedElement".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.import_imported_element".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::Membership".to_string(),
            feature: "memberElementId".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.membership_member_element_id".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Root::AnnotatingElement".to_string(),
            feature: "annotation".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.annotating_element_annotation".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Feature".to_string(),
            feature: "chainingFeature".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.feature_chaining_feature".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Feature".to_string(),
            feature: "crossFeature".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.feature_cross_feature".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Feature".to_string(),
            feature: "featureTarget".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.feature_target".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Feature".to_string(),
            feature: "featuringType".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.feature_featuring_type".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Type".to_string(),
            feature: "differencingType".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.type_differencing_type".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Type".to_string(),
            feature: "featureMembership".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.type_feature_membership".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Type".to_string(),
            feature: "intersectingType".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.type_intersecting_type".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Type".to_string(),
            feature: "unioningType".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.type_unioning_type".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Core::Type".to_string(),
            feature: "isConjugated".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.type_is_conjugated".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Kernel::Flow".to_string(),
            feature: "payloadType".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.flow_payload_type".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Kernel::Flow".to_string(),
            feature: "sourceOutputFeature".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.flow_source_output_feature".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Core::Kernel::Flow".to_string(),
            feature: "targetInputFeature".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "core.flow_target_input_feature".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Model::Systems::RequirementDefinition".to_string(),
            feature: "text".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "model.requirement_text".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Model::Systems::RequirementUsage".to_string(),
            feature: "text".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "model.requirement_text".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "Model::Systems::Usage".to_string(),
            feature: "isReference".to_string(),
            rule: DerivedFeatureRule::Native {
                function: "model.usage_is_reference".to_string(),
            },
        },
        DerivedFeatureSpec {
            owner: "*".to_string(),
            feature: "name".to_string(),
            rule: DerivedFeatureRule::Name,
        },
        DerivedFeatureSpec {
            owner: "*".to_string(),
            feature: "shortName".to_string(),
            rule: DerivedFeatureRule::ShortName,
        },
        DerivedFeatureSpec {
            owner: "*".to_string(),
            feature: "qualifiedName".to_string(),
            rule: DerivedFeatureRule::QualifiedName,
        },
        DerivedFeatureSpec {
            owner: "*".to_string(),
            feature: "isLibraryElement".to_string(),
            rule: DerivedFeatureRule::LibraryBoolean,
        },
    ]
}

fn spec_matches_element(spec: &DerivedFeatureSpec, element: &Element) -> bool {
    spec.owner == "*"
        || element.kind.as_ref() == spec.owner
        || element.kind.ends_with(&format!("::{}", spec.owner))
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == spec.owner)
        || property_refs(&element.properties, "metatype")
            .into_iter()
            .any(|metatype| metatype == spec.owner)
        || (spec.owner == "Core::Root::Element" && element.layer <= 2)
}

fn target_matches(
    element: &Element,
    target_kind: &Option<String>,
    target_type: &Option<String>,
) -> bool {
    let kind_matches = target_kind.as_deref().is_none_or(|target| {
        element.kind.as_ref() == target || element.kind.ends_with(&format!("::{target}"))
    });
    let type_matches = target_type.as_deref().is_none_or(|target| {
        property_refs(&element.properties, "type")
            .into_iter()
            .chain(property_refs(&element.properties, "metatype"))
            .any(|type_ref| type_ref == target || type_ref.ends_with(&format!("::{target}")))
    });
    kind_matches && type_matches
}

fn feature_name(value: &str) -> &str {
    value.rsplit("::").next().unwrap_or(value)
}

fn element_specializes(graph: &Graph, element: &Element, target: &str) -> bool {
    if element.element_id == target {
        return true;
    }
    if property_refs(&element.properties, "specializes")
        .into_iter()
        .any(|specialized| specialized == target)
    {
        return true;
    }

    let mut seen = BTreeSet::new();
    let mut stack = graph
        .outgoing(element.id, "specializes")
        .map(|edge| edge.target)
        .collect::<Vec<_>>();
    while let Some(node_id) = stack.pop() {
        if !seen.insert(node_id) {
            continue;
        }
        let Some(candidate) = graph.element(node_id) else {
            continue;
        };
        if candidate.element_id == target {
            return true;
        }
        stack.extend(
            graph
                .outgoing(node_id, "specializes")
                .map(|edge| edge.target),
        );
    }

    false
}

fn refs_from_value(value: &Value) -> Vec<String> {
    match value {
        Value::String(value) => vec![value.clone()],
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn intersect_source_refs(
    registry: &DerivedFeatureRegistry,
    graph: &Graph,
    element: &Element,
    sources: &[String],
) -> Option<Vec<String>> {
    let (first_source, remaining_sources) = sources.split_first()?;
    let first_value =
        registry.derived_property_uncached(graph, element, feature_name(first_source))?;
    let mut ordered_ids = refs_from_value(&first_value.value);
    for source in remaining_sources {
        let source_value =
            registry.derived_property_uncached(graph, element, feature_name(source))?;
        let source_ids = refs_from_value(&source_value.value)
            .into_iter()
            .collect::<BTreeSet<_>>();
        ordered_ids.retain(|id| source_ids.contains(id));
    }
    Some(ordered_ids)
}

fn relation_targets(graph: &Graph, element: &Element, relation: &str) -> Vec<String> {
    graph
        .outgoing(element.id, relation)
        .filter_map(|edge| graph.element_id(edge.target))
        .map(str::to_string)
        .collect()
}

fn is_native_function(function: &str) -> bool {
    matches!(
        function,
        "core.owner"
            | "core.owned_element"
            | "core.documentation"
            | "core.documented_element"
            | "core.annotated_element"
            | "core.owning_namespace"
            | "core.namespace_member"
            | "core.namespace_membership"
            | "core.relationship_related_element"
            | "core.import_imported_element"
            | "core.membership_member_element_id"
            | "core.annotating_element_annotation"
            | "core.feature_chaining_feature"
            | "core.feature_cross_feature"
            | "core.feature_target"
            | "core.feature_featuring_type"
            | "core.type_differencing_type"
            | "core.type_feature_membership"
            | "core.type_intersecting_type"
            | "core.type_unioning_type"
            | "core.type_is_conjugated"
            | "core.flow_payload_type"
            | "core.flow_source_output_feature"
            | "core.flow_target_input_feature"
            | "model.requirement_text"
            | "model.usage_is_reference"
            | "core.name"
            | "core.short_name"
            | "core.qualified_name"
            | "core.is_library_element"
    )
}

fn derive_native_function(
    function: &str,
    graph: &Graph,
    element: &Element,
) -> Option<DerivedPropertyValue> {
    match function {
        "core.owner" => derived_owner(graph, element),
        "core.owned_element" => derived_owned_element(graph, element),
        "core.documentation" => derived_documentation(graph, element),
        "core.documented_element" if is_documentation_element(element) => {
            derived_owner(graph, element)
        }
        "core.annotated_element" if is_documentation_element(element) => {
            derived_owner(graph, element)
        }
        "core.owning_namespace" => derived_owning_namespace(graph, element),
        "core.namespace_member" => derived_namespace_member(graph, element),
        "core.namespace_membership" => derived_namespace_membership(graph, element),
        "core.relationship_related_element" => derived_relationship_related_element(graph, element),
        "core.import_imported_element" => derived_import_imported_element(graph, element),
        "core.membership_member_element_id" => derived_membership_member_element_id(graph, element),
        "core.annotating_element_annotation" => {
            derived_annotating_element_annotation(graph, element)
        }
        "core.feature_chaining_feature" => derived_feature_chaining_feature(graph, element),
        "core.feature_cross_feature" => derived_feature_cross_feature(graph, element),
        "core.feature_target" => derived_feature_target(graph, element),
        "core.feature_featuring_type" => derived_feature_featuring_type(graph, element),
        "core.type_differencing_type" => derived_type_role_type(
            graph,
            element,
            "Core::Core::Differencing",
            "differencingType",
            "ownedDifferencing",
        ),
        "core.type_feature_membership" => derived_type_feature_membership(graph, element),
        "core.type_intersecting_type" => derived_type_role_type(
            graph,
            element,
            "Core::Core::Intersecting",
            "intersectingType",
            "ownedIntersecting",
        ),
        "core.type_unioning_type" => derived_type_role_type(
            graph,
            element,
            "Core::Core::Unioning",
            "unioningType",
            "ownedUnioning",
        ),
        "core.type_is_conjugated" => derived_type_is_conjugated(graph, element),
        "core.flow_payload_type" => derived_flow_payload_type(graph, element),
        "core.flow_source_output_feature" => derived_flow_end_owned_feature(graph, element, 0),
        "core.flow_target_input_feature" => derived_flow_end_owned_feature(graph, element, 1),
        "model.requirement_text" => derived_requirement_text(graph, element),
        "model.usage_is_reference" => derived_usage_is_reference(element),
        "core.name" => derived_name(element),
        "core.short_name" => derived_short_name(element),
        "core.qualified_name" => derived_qualified_name(graph, element),
        "core.is_library_element" => Some(DerivedPropertyValue {
            value: Value::Bool(element.layer < 2),
            source: DerivedPropertySource::Layer,
        }),
        _ => None,
    }
}

fn derived_owner(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    if let Some(value) = element.properties.get("owner") {
        return Some(DerivedPropertyValue {
            value: value.clone(),
            source: DerivedPropertySource::ExplicitProperty,
        });
    }

    for relation in ["members", "features", "ownedElement"] {
        if let Some(edge) = graph.incoming(element.id, relation).next()
            && let Some(owner_id) = graph.element_id(edge.source)
        {
            return Some(DerivedPropertyValue {
                value: Value::String(owner_id.to_string()),
                source: DerivedPropertySource::InverseRelation,
            });
        }
    }

    None
}

fn derived_owned_element(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let mut ids: Vec<Value> = Vec::new();
    for relation in ["members", "features", "ownedElement"] {
        for edge in graph.outgoing(element.id, relation) {
            let Some(target) = graph.element(edge.target) else {
                continue;
            };
            if is_namespace_membership_element(target) || is_import_element(target) {
                continue;
            }
            if !ids
                .iter()
                .any(|existing| existing.as_str() == Some(&target.element_id))
            {
                ids.push(Value::String(target.element_id.clone()));
            }
        }
    }

    let had_forward_relations = !ids.is_empty();
    for child_id in owned_children_from_owner(graph, element, Some(is_owned_element_candidate)) {
        if !ids
            .iter()
            .any(|existing| existing.as_str() == Some(&child_id))
        {
            ids.push(Value::String(child_id));
        }
    }

    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: Value::Array(ids),
        source: if had_forward_relations {
            DerivedPropertySource::ForwardRelation
        } else {
            DerivedPropertySource::InverseRelation
        },
    })
}

fn derived_documentation(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let ids = owned_children_from_owner(graph, element, Some(is_documentation_element));
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::OwnedElementSubset,
    })
}

fn derived_owning_namespace(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let owner_id = derived_owner_id(graph, element)?;
    let owner = graph.element_by_element_id(&owner_id)?;
    if is_namespace_element(owner) {
        return Some(DerivedPropertyValue {
            value: Value::String(owner_id),
            source: DerivedPropertySource::ManifestNative,
        });
    }
    derived_owning_namespace(graph, owner)
}

fn derived_namespace_member(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let mut ids = relation_targets(graph, element, "members");
    ids.extend(relation_targets(graph, element, "member"));
    ids = unique_strings(ids);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_namespace_membership(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let ids = relation_targets(graph, element, "members")
        .into_iter()
        .chain(relation_targets(graph, element, "membership"))
        .filter(|id| {
            graph.element_by_element_id(id).is_some_and(|candidate| {
                candidate.kind.as_ref() == "Core::Root::Membership"
                    || candidate.kind.ends_with("::Membership")
                    || element_specializes(graph, candidate, "Core::Root::Membership")
            })
        })
        .collect::<Vec<_>>();
    let ids = unique_strings(ids);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_relationship_related_element(
    graph: &Graph,
    element: &Element,
) -> Option<DerivedPropertyValue> {
    let mut ids = relation_targets(graph, element, "source");
    ids.extend(relation_targets(graph, element, "target"));
    ids.extend(relation_targets(graph, element, "relatedElement"));
    let ids = unique_strings(ids);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_import_imported_element(
    graph: &Graph,
    element: &Element,
) -> Option<DerivedPropertyValue> {
    let mut ids = relation_targets(graph, element, "target");
    ids.extend(relation_targets(graph, element, "imports"));
    ids.extend(relation_targets(graph, element, "importedElement"));
    let ids = unique_strings(ids);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_membership_member_element_id(
    graph: &Graph,
    element: &Element,
) -> Option<DerivedPropertyValue> {
    element
        .properties
        .get("memberElement")
        .or_else(|| element.properties.get("target"))
        .and_then(Value::as_str)
        .map(|id| DerivedPropertyValue {
            value: Value::String(id.to_string()),
            source: DerivedPropertySource::ManifestNative,
        })
        .or_else(|| {
            graph
                .outgoing(element.id, "memberElement")
                .chain(graph.outgoing(element.id, "target"))
                .next()
                .and_then(|edge| graph.element_id(edge.target))
                .map(|id| DerivedPropertyValue {
                    value: Value::String(id.to_string()),
                    source: DerivedPropertySource::ManifestNative,
                })
        })
}

fn derived_annotating_element_annotation(
    graph: &Graph,
    element: &Element,
) -> Option<DerivedPropertyValue> {
    let mut ids = direct_and_edge_refs(graph, element, "owningAnnotatingRelationship");
    ids.extend(direct_and_edge_refs(
        graph,
        element,
        "ownedAnnotatingRelationship",
    ));
    ids.extend(direct_and_edge_refs(graph, element, "ownedAnnotation"));
    ids.extend(direct_and_edge_refs(graph, element, "annotation"));
    let ids = unique_strings(ids);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_feature_chaining_feature(
    graph: &Graph,
    element: &Element,
) -> Option<DerivedPropertyValue> {
    let ids = role_targets_from_owned_relationships(
        graph,
        element,
        "Core::Core::FeatureChaining",
        "chainingFeature",
        "ownedFeatureChaining",
    );
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_feature_cross_feature(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let crossed_feature_id = role_targets_from_owned_relationships(
        graph,
        element,
        "Core::Core::CrossSubsetting",
        "crossedFeature",
        "ownedCrossSubsetting",
    )
    .into_iter()
    .next()?;
    let crossed_feature = graph.element_by_element_id(&crossed_feature_id)?;
    let chaining_features = feature_chaining_feature_refs(graph, crossed_feature);
    chaining_features
        .get(1)
        .cloned()
        .map(|id| DerivedPropertyValue {
            value: Value::String(id),
            source: DerivedPropertySource::ManifestNative,
        })
}

fn derived_feature_target(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let target = feature_chaining_feature_refs(graph, element)
        .into_iter()
        .last()
        .unwrap_or_else(|| element.element_id.clone());
    Some(DerivedPropertyValue {
        value: Value::String(target),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_feature_featuring_type(
    graph: &Graph,
    element: &Element,
) -> Option<DerivedPropertyValue> {
    let mut seen = BTreeSet::new();
    let ids = feature_featuring_type_refs(graph, element, &mut seen);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_type_role_type(
    graph: &Graph,
    element: &Element,
    relationship_type: &str,
    role: &str,
    owned_role: &str,
) -> Option<DerivedPropertyValue> {
    let ids =
        role_targets_from_owned_relationships(graph, element, relationship_type, role, owned_role);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn feature_chaining_feature_refs(graph: &Graph, element: &Element) -> Vec<String> {
    role_targets_from_owned_relationships(
        graph,
        element,
        "Core::Core::FeatureChaining",
        "chainingFeature",
        "ownedFeatureChaining",
    )
}

fn feature_featuring_type_refs(
    graph: &Graph,
    element: &Element,
    seen: &mut BTreeSet<String>,
) -> Vec<String> {
    if !seen.insert(element.element_id.clone()) {
        return Vec::new();
    }

    let mut ids = role_targets_from_owned_relationships(
        graph,
        element,
        "Core::Core::TypeFeaturing",
        "featuringType",
        "ownedTypeFeaturing",
    );
    if let Some(first_chaining_feature_id) = feature_chaining_feature_refs(graph, element).first()
        && let Some(first_chaining_feature) = graph.element_by_element_id(first_chaining_feature_id)
    {
        ids.extend(feature_featuring_type_refs(
            graph,
            first_chaining_feature,
            seen,
        ));
    }
    unique_strings(ids)
}

fn derived_type_feature_membership(
    graph: &Graph,
    element: &Element,
) -> Option<DerivedPropertyValue> {
    let mut ids = direct_and_edge_refs(graph, element, "featureMembership");
    ids.extend(direct_and_edge_refs(
        graph,
        element,
        "ownedFeatureMembership",
    ));
    ids.extend(
        owned_children_from_owner(graph, element, Some(is_feature_membership_element)).into_iter(),
    );
    let ids = unique_strings(ids);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_type_is_conjugated(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let has_conjugator = !direct_and_edge_refs(graph, element, "ownedConjugator").is_empty()
        || owned_children_from_owner(graph, element, Some(is_conjugation_element))
            .into_iter()
            .next()
            .is_some();
    Some(DerivedPropertyValue {
        value: Value::Bool(has_conjugator),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_flow_payload_type(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let payload_features = direct_and_edge_refs(graph, element, "payloadFeature")
        .into_iter()
        .chain(owned_feature_refs_specializing(
            graph,
            element,
            "Core::Kernel::Flow::payloadFeature",
        ))
        .chain(
            owned_children_from_owner(graph, element, None)
                .into_iter()
                .filter(|id| {
                    graph.element_by_element_id(id).is_some_and(|feature| {
                        element_specializes(graph, feature, "Core::Kernel::Flow::payloadFeature")
                    })
                }),
        )
        .collect::<Vec<_>>();
    let mut ids = Vec::new();
    for payload_feature_id in payload_features {
        let Some(payload_feature) = graph.element_by_element_id(&payload_feature_id) else {
            continue;
        };
        ids.extend(direct_and_edge_refs(graph, payload_feature, "type"));
    }
    let ids = unique_strings(ids);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_requirement_text(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let text = owned_children_from_owner(graph, element, Some(is_documentation_element))
        .into_iter()
        .filter_map(|id| graph.element_by_element_id(&id))
        .filter_map(|doc| doc.properties.get("body").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!text.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_strings(text),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_usage_is_reference(element: &Element) -> Option<DerivedPropertyValue> {
    let is_composite = element
        .properties
        .get("isComposite")
        .or_else(|| element.properties.get("is_composite"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(DerivedPropertyValue {
        value: Value::Bool(!is_composite),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn derived_flow_end_owned_feature(
    graph: &Graph,
    element: &Element,
    end_index: usize,
) -> Option<DerivedPropertyValue> {
    let connector_ends = direct_and_edge_refs(graph, element, "connectorEnd")
        .into_iter()
        .chain(owned_feature_refs_specializing(
            graph,
            element,
            "Core::Kernel::Connector::connectorEnd",
        ))
        .collect::<Vec<_>>();
    let connector_end_id = connector_ends.get(end_index)?;
    let connector_end = graph.element_by_element_id(connector_end_id)?;
    let ids = direct_and_edge_refs(graph, connector_end, "ownedFeature")
        .into_iter()
        .chain(direct_and_edge_refs(graph, connector_end, "features"))
        .collect::<Vec<_>>();
    let ids = unique_strings(ids);
    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: value_for_refs(ids),
        source: DerivedPropertySource::ManifestNative,
    })
}

fn role_targets_from_owned_relationships(
    graph: &Graph,
    element: &Element,
    relationship_type: &str,
    role: &str,
    owned_role: &str,
) -> Vec<String> {
    let relationship_ids = direct_and_edge_refs(graph, element, owned_role)
        .into_iter()
        .chain(owned_children_from_owner(
            graph,
            element,
            Some(is_relationship_element),
        ))
        .collect::<Vec<_>>();
    let mut ids = Vec::new();
    for relationship_id in relationship_ids {
        let Some(relationship) = graph.element_by_element_id(&relationship_id) else {
            continue;
        };
        if relationship_matches(graph, relationship, relationship_type) {
            ids.extend(direct_and_edge_refs(graph, relationship, role));
        }
    }
    unique_strings(ids)
}

fn owned_feature_refs_specializing(
    graph: &Graph,
    element: &Element,
    target_feature: &str,
) -> Vec<String> {
    direct_and_edge_refs(graph, element, "ownedFeature")
        .into_iter()
        .chain(direct_and_edge_refs(graph, element, "features"))
        .filter(|id| {
            graph
                .element_by_element_id(id)
                .is_some_and(|feature| element_specializes(graph, feature, target_feature))
        })
        .collect()
}

fn direct_and_edge_refs(graph: &Graph, element: &Element, relation: &str) -> Vec<String> {
    property_refs(&element.properties, relation)
        .into_iter()
        .map(str::to_string)
        .chain(relation_targets(graph, element, relation))
        .collect()
}

fn owned_children_from_owner(
    graph: &Graph,
    element: &Element,
    filter: Option<fn(&Element) -> bool>,
) -> Vec<String> {
    let mut ids = Vec::new();
    for edge in graph.incoming(element.id, "owner") {
        let Some(child) = graph.element(edge.source) else {
            continue;
        };
        if filter.is_none_or(|filter| filter(child)) {
            ids.push(child.element_id.clone());
        }
    }
    ids
}

fn unique_strings(values: Vec<String>) -> Vec<String> {
    values.into_iter().fold(Vec::new(), |mut unique, value| {
        if !unique.iter().any(|existing| existing == &value) {
            unique.push(value);
        }
        unique
    })
}

fn value_for_refs(ids: Vec<String>) -> Value {
    match ids.as_slice() {
        [only] => Value::String(only.clone()),
        _ => Value::Array(ids.into_iter().map(Value::String).collect()),
    }
}

fn value_for_strings(values: Vec<String>) -> Value {
    match values.as_slice() {
        [only] => Value::String(only.clone()),
        _ => Value::Array(values.into_iter().map(Value::String).collect()),
    }
}

fn is_namespace_element(element: &Element) -> bool {
    element.kind.as_ref() == "Core::Root::Namespace"
        || element.kind.ends_with("::Namespace")
        || element.kind.as_ref() == "Namespace"
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == "Core::Root::Namespace")
}

fn is_documentation_element(element: &Element) -> bool {
    element.kind.as_ref() == "Core::Root::Documentation"
        || element.kind.ends_with("::Documentation")
        || element.kind.as_ref() == "Documentation"
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == "Core::Root::Documentation")
}

fn is_relationship_element(element: &Element) -> bool {
    element.kind.as_ref() == "Core::Root::Relationship"
        || element.kind.ends_with("::Relationship")
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == "Core::Root::Relationship")
}

fn is_feature_membership_element(element: &Element) -> bool {
    element.kind.as_ref() == "Core::Core::FeatureMembership"
        || element.kind.ends_with("::FeatureMembership")
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == "Core::Core::FeatureMembership")
}

fn is_owned_element_candidate(element: &Element) -> bool {
    !is_namespace_membership_element(element) && !is_import_element(element)
}

fn is_namespace_membership_element(element: &Element) -> bool {
    element.kind.as_ref() == "Core::Root::Membership"
        || element.kind.as_ref() == "Membership"
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == "Core::Root::Membership")
}

fn is_import_element(element: &Element) -> bool {
    element.kind.as_ref() == "Core::Root::Import"
        || element.kind.ends_with("::Import")
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == "Core::Root::Import" || type_ref.ends_with("::Import"))
}

fn is_conjugation_element(element: &Element) -> bool {
    element.kind.as_ref() == "Core::Core::Conjugation"
        || element.kind.ends_with("::Conjugation")
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == "Core::Core::Conjugation")
}

fn relationship_matches(graph: &Graph, element: &Element, target: &str) -> bool {
    element.kind.as_ref() == target
        || element.kind.ends_with(&format!("::{target}"))
        || property_refs(&element.properties, "type")
            .into_iter()
            .any(|type_ref| type_ref == target)
        || element_specializes(graph, element, target)
}

fn property_refs<'a>(properties: &'a ElementProperties, key: &str) -> Vec<&'a str> {
    match properties.get(key) {
        Some(Value::String(value)) => vec![value.as_str()],
        Some(Value::Array(values)) => values.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn derived_name(element: &Element) -> Option<DerivedPropertyValue> {
    element
        .properties
        .get("declared_name")
        .cloned()
        .map(|value| DerivedPropertyValue {
            value,
            source: DerivedPropertySource::DeclaredName,
        })
}

fn derived_short_name(element: &Element) -> Option<DerivedPropertyValue> {
    element
        .properties
        .get("declared_short_name")
        .cloned()
        .map(|value| DerivedPropertyValue {
            value,
            source: DerivedPropertySource::DeclaredShortName,
        })
}

fn derived_qualified_name(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let mut segments = Vec::new();
    let mut current = element;
    let mut seen = BTreeSet::new();

    loop {
        if !seen.insert(current.element_id.clone()) {
            return None;
        }
        segments.push(local_name(current)?);
        let Some(owner_id) = derived_owner_id(graph, current) else {
            break;
        };
        current = graph.element_by_element_id(&owner_id)?;
    }

    segments.reverse();
    Some(DerivedPropertyValue {
        value: Value::String(segments.join("::")),
        source: DerivedPropertySource::OwnerNameChain,
    })
}

fn local_name(element: &Element) -> Option<String> {
    element
        .properties
        .get("declared_name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            element
                .properties
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn derived_owner_id(graph: &Graph, element: &Element) -> Option<String> {
    match derived_owner(graph, element)?.value {
        Value::String(owner_id) => Some(owner_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::model::ir::{KirDocument, KirElement};

    #[test]
    fn derives_owner_and_owned_element_from_graph_relations() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "features".to_string(),
                        json!(["feature.Demo.Vehicle.engine"]),
                    )]),
                },
                KirElement {
                    id: "feature.Demo.Vehicle.engine".to_string(),
                    kind: "Model::Parts::PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "declared_name".to_string(),
                        Value::String("engine".to_string()),
                    )]),
                },
            ],
        })
        .unwrap();

        let owner = graph.element_by_element_id("type.Demo.Vehicle").unwrap();
        let child = graph
            .element_by_element_id("feature.Demo.Vehicle.engine")
            .unwrap();
        let owner_derived = derived_properties(&graph, owner);
        let child_derived = derived_properties(&graph, child);

        assert_eq!(
            owner_derived.get("ownedElement").map(|value| &value.value),
            Some(&json!(["feature.Demo.Vehicle.engine"]))
        );
        assert_eq!(
            child_derived.get("owner").map(|value| &value.value),
            Some(&Value::String("type.Demo.Vehicle".to_string()))
        );
        assert_eq!(
            child_derived.get("name").map(|value| &value.value),
            Some(&Value::String("engine".to_string()))
        );
    }

    #[test]
    fn derives_qualified_name_from_owner_name_chain_only() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.generated.1".to_string(),
                    kind: "Model::Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "declared_name".to_string(),
                        Value::String("Demo".to_string()),
                    )]),
                },
                KirElement {
                    id: "type.generated.2".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "declared_name".to_string(),
                            Value::String("Vehicle".to_string()),
                        ),
                        (
                            "owner".to_string(),
                            Value::String("pkg.generated.1".to_string()),
                        ),
                    ]),
                },
            ],
        })
        .unwrap();

        let element = graph.element_by_element_id("type.generated.2").unwrap();
        let derived = derived_properties(&graph, element);

        assert_eq!(
            derived.get("qualifiedName").map(|value| &value.value),
            Some(&Value::String("Demo::Vehicle".to_string()))
        );
    }

    #[test]
    fn omits_qualified_name_when_owner_name_chain_is_incomplete() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::new(),
            }],
        })
        .unwrap();

        let element = graph.element_by_element_id("type.Demo.Vehicle").unwrap();
        let derived = derived_properties(&graph, element);

        assert!(!derived.contains_key("qualifiedName"));
    }

    #[test]
    fn derives_documentation_from_owned_documentation_elements() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.A".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "features".to_string(),
                        json!(["feature.Demo.A.x"]),
                    )]),
                },
                KirElement {
                    id: "feature.Demo.A.x".to_string(),
                    kind: "Model::Systems::PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "doc.type.Demo.A.1".to_string(),
                    kind: "Core::Root::Documentation".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "owner".to_string(),
                            Value::String("type.Demo.A".to_string()),
                        ),
                        ("body".to_string(), Value::String("doc from A".to_string())),
                    ]),
                },
            ],
        })
        .unwrap();
        let cache = DerivedFeatureCache::new("revision-7");
        let registry = DerivedFeatureRegistry::with_builtin_core_specs();
        let owner = graph.element_by_element_id("type.Demo.A").unwrap();
        let documentation = graph.element_by_element_id("doc.type.Demo.A.1").unwrap();

        assert_eq!(cache.revision(), "revision-7");
        assert_eq!(
            cache
                .derived_property(&registry, &graph, owner, "documentation")
                .map(|value| value.value),
            Some(Value::String("doc.type.Demo.A.1".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(&registry, &graph, owner, "ownedElement")
                .map(|value| value.value),
            Some(json!(["feature.Demo.A.x", "doc.type.Demo.A.1"]))
        );
        assert_eq!(
            cache
                .derived_property(&registry, &graph, documentation, "documentedElement")
                .map(|value| value.value),
            Some(Value::String("type.Demo.A".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(&registry, &graph, documentation, "annotatedElement")
                .map(|value| value.value),
            Some(Value::String("type.Demo.A".to_string()))
        );
    }

    #[test]
    fn owned_element_excludes_namespace_membership_relationships() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.Demo".to_string(),
                    kind: "Core::Root::Namespace".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("members".to_string(), json!(["type.Demo.A"])),
                        ("membership".to_string(), json!(["member.Demo.A"])),
                    ]),
                },
                KirElement {
                    id: "member.Demo.A".to_string(),
                    kind: "Core::Root::Membership".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("pkg.Demo")),
                        ("memberElement".to_string(), json!("type.Demo.A")),
                    ]),
                },
                KirElement {
                    id: "type.Demo.A".to_string(),
                    kind: "Core::Root::Element".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([("owner".to_string(), json!("pkg.Demo"))]),
                },
                KirElement {
                    id: "import.Demo.A".to_string(),
                    kind: "Core::Root::Import".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("pkg.Demo")),
                        ("target".to_string(), json!("type.Demo.A")),
                    ]),
                },
            ],
        })
        .unwrap();
        let registry = DerivedFeatureRegistry::with_builtin_core_specs();
        let cache = DerivedFeatureCache::new("revision-membership");
        let package = graph.element_by_element_id("pkg.Demo").unwrap();

        assert_eq!(
            cache
                .derived_property(&registry, &graph, package, "ownedElement")
                .map(|value| value.value),
            Some(json!(["type.Demo.A"]))
        );
        assert_eq!(
            cache
                .derived_property(&registry, &graph, package, "membership")
                .map(|value| value.value),
            Some(json!(["member.Demo.A"]))
        );
    }

    #[test]
    fn loads_manifest_from_merged_document_metadata() {
        let metadata = BTreeMap::from([(
            "merged_sources".to_string(),
            json!([
                {
                    "source": "stdlib",
                    "derived_feature_manifest": {
                        "metamodel": "test",
                        "derived_features": [
                            {
                                "owner": "*",
                                "feature": "label",
                                "kind": "name"
                            }
                        ]
                    }
                }
            ]),
        )]);

        let manifest = manifest_from_metadata(&metadata).unwrap().unwrap();

        assert_eq!(manifest.metamodel.as_deref(), Some("test"));
        assert_eq!(manifest.derived_features.len(), 1);
        assert_eq!(manifest.derived_features[0].feature, "label");
    }

    #[test]
    fn manifest_backed_registry_derives_custom_subset_feature() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.A".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "ownedElement".to_string(),
                        json!(["doc.type.Demo.A.1", "feature.Demo.A.x"]),
                    )]),
                },
                KirElement {
                    id: "doc.type.Demo.A.1".to_string(),
                    kind: "Core::Root::Documentation".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "feature.Demo.A.x".to_string(),
                    kind: "Model::Systems::PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();
        let registry =
            DerivedFeatureRegistry::with_manifest_and_builtins(Some(DerivedFeatureManifest {
                metamodel: Some("test".to_string()),
                derived_features: vec![DerivedFeatureSpec {
                    owner: "*".to_string(),
                    feature: "primaryDoc".to_string(),
                    rule: DerivedFeatureRule::Subset {
                        source: "ownedElement".to_string(),
                        target_kind: Some("Core::Root::Documentation".to_string()),
                        target_type: None,
                    },
                }],
            }))
            .unwrap();
        let cache = DerivedFeatureCache::new("revision-8");
        let owner = graph.element_by_element_id("type.Demo.A").unwrap();

        assert_eq!(
            cache
                .derived_property(&registry, &graph, owner, "primaryDoc")
                .map(|value| value.value),
            Some(Value::String("doc.type.Demo.A.1".to_string()))
        );
    }

    #[test]
    fn manifest_backed_registry_derives_subset_chain_feature() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.A".to_string(),
                    kind: "Model::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "ownedElement".to_string(),
                        json!(["rel.Demo.A.r1", "feature.Demo.A.x"]),
                    )]),
                },
                KirElement {
                    id: "rel.Demo.A.r1".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        json!(["Core::Root::Element::ownedRelationship"]),
                    )]),
                },
                KirElement {
                    id: "feature.Demo.A.x".to_string(),
                    kind: "Model::Systems::PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();
        let registry =
            DerivedFeatureRegistry::with_manifest_and_builtins(Some(DerivedFeatureManifest {
                metamodel: Some("test".to_string()),
                derived_features: vec![DerivedFeatureSpec {
                    owner: "*".to_string(),
                    feature: "ownedRelationship".to_string(),
                    rule: DerivedFeatureRule::SubsetChain {
                        source: "Core::Root::Element::ownedElement".to_string(),
                        target_feature: "Core::Root::Element::ownedRelationship".to_string(),
                        target_kind: None,
                        target_type: None,
                    },
                }],
            }))
            .unwrap();
        let cache = DerivedFeatureCache::new("revision-10");
        let owner = graph.element_by_element_id("type.Demo.A").unwrap();

        assert_eq!(
            cache
                .derived_property(&registry, &graph, owner, "ownedRelationship")
                .map(|value| value.value),
            Some(Value::String("rel.Demo.A.r1".to_string()))
        );
    }

    #[test]
    fn manifest_backed_registry_derives_intersection_subset_chain_feature() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.A".to_string(),
                    kind: "Core::Core::Type".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "ownedFeature".to_string(),
                            json!(["feature.Demo.A.end", "feature.Demo.A.ownedOnly"]),
                        ),
                        (
                            "endFeature".to_string(),
                            json!(["feature.Demo.A.end", "feature.Demo.A.endOnly"]),
                        ),
                    ]),
                },
                KirElement {
                    id: "feature.Demo.A.end".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        json!(["Core::Core::Type::ownedEndFeature"]),
                    )]),
                },
                KirElement {
                    id: "feature.Demo.A.ownedOnly".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "feature.Demo.A.endOnly".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        })
        .unwrap();
        let registry =
            DerivedFeatureRegistry::with_manifest_and_builtins(Some(DerivedFeatureManifest {
                metamodel: Some("test".to_string()),
                derived_features: vec![DerivedFeatureSpec {
                    owner: "*".to_string(),
                    feature: "ownedEndFeature".to_string(),
                    rule: DerivedFeatureRule::IntersectionSubsetChain {
                        sources: vec![
                            "Core::Core::Type::endFeature".to_string(),
                            "Core::Core::Type::ownedFeature".to_string(),
                        ],
                        target_feature: "Core::Core::Type::ownedEndFeature".to_string(),
                        target_kind: None,
                        target_type: None,
                    },
                }],
            }))
            .unwrap();
        let cache = DerivedFeatureCache::new("revision-11");
        let owner = graph.element_by_element_id("type.Demo.A").unwrap();

        assert_eq!(
            cache
                .derived_property(&registry, &graph, owner, "ownedEndFeature")
                .map(|value| value.value),
            Some(Value::String("feature.Demo.A.end".to_string()))
        );
    }

    #[test]
    fn core_native_structural_features_derive_from_direct_relationships() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.Demo".to_string(),
                    kind: "Core::Root::Namespace".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "members".to_string(),
                        json!(["member.Demo.a", "rel.Demo.r"]),
                    )]),
                },
                KirElement {
                    id: "member.Demo.a".to_string(),
                    kind: "Core::Root::Membership".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "memberElement".to_string(),
                        json!("type.Demo.A"),
                    )]),
                },
                KirElement {
                    id: "type.Demo.A".to_string(),
                    kind: "Core::Root::Element".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([("owner".to_string(), json!("pkg.Demo"))]),
                },
                KirElement {
                    id: "type.Demo.B".to_string(),
                    kind: "Core::Root::Element".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "rel.Demo.r".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("source".to_string(), json!("type.Demo.A")),
                        ("target".to_string(), json!("type.Demo.B")),
                    ]),
                },
                KirElement {
                    id: "import.Demo.B".to_string(),
                    kind: "Core::Root::Import".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([("target".to_string(), json!("type.Demo.B"))]),
                },
                KirElement {
                    id: "type.Demo.T".to_string(),
                    kind: "Core::Core::Type".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "rel.Demo.T.union".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("type.Demo.T")),
                        ("type".to_string(), json!("Core::Core::Unioning")),
                        ("unioningType".to_string(), json!("type.Demo.A")),
                    ]),
                },
                KirElement {
                    id: "rel.Demo.T.intersect".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("type.Demo.T")),
                        ("type".to_string(), json!("Core::Core::Intersecting")),
                        ("intersectingType".to_string(), json!("type.Demo.B")),
                    ]),
                },
                KirElement {
                    id: "rel.Demo.T.diff".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("type.Demo.T")),
                        ("type".to_string(), json!("Core::Core::Differencing")),
                        ("differencingType".to_string(), json!("type.Demo.B")),
                    ]),
                },
                KirElement {
                    id: "member.Demo.T.feature".to_string(),
                    kind: "Core::Core::FeatureMembership".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([("owner".to_string(), json!("type.Demo.T"))]),
                },
                KirElement {
                    id: "rel.Demo.T.conjugator".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("type.Demo.T")),
                        ("type".to_string(), json!("Core::Core::Conjugation")),
                    ]),
                },
                KirElement {
                    id: "feature.Demo.chain".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "rel.Demo.chain".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("feature.Demo.chain")),
                        ("type".to_string(), json!("Core::Core::FeatureChaining")),
                        ("chainingFeature".to_string(), json!("feature.Demo.target")),
                    ]),
                },
                KirElement {
                    id: "feature.Demo.target".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "feature.Demo.withTypeFeaturing".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "rel.Demo.withTypeFeaturing".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("feature.Demo.withTypeFeaturing")),
                        ("type".to_string(), json!("Core::Core::TypeFeaturing")),
                        ("featuringType".to_string(), json!("type.Demo.T")),
                    ]),
                },
                KirElement {
                    id: "feature.Demo.twoChain".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "rel.Demo.twoChain.1".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("feature.Demo.twoChain")),
                        ("type".to_string(), json!("Core::Core::FeatureChaining")),
                        ("chainingFeature".to_string(), json!("feature.Demo.source")),
                    ]),
                },
                KirElement {
                    id: "rel.Demo.twoChain.2".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("feature.Demo.twoChain")),
                        ("type".to_string(), json!("Core::Core::FeatureChaining")),
                        (
                            "chainingFeature".to_string(),
                            json!("feature.Demo.targetInput"),
                        ),
                    ]),
                },
                KirElement {
                    id: "feature.Demo.crossing".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "rel.Demo.crossing".to_string(),
                    kind: "Core::Root::Relationship".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("feature.Demo.crossing")),
                        ("type".to_string(), json!("Core::Core::CrossSubsetting")),
                        ("crossedFeature".to_string(), json!("feature.Demo.twoChain")),
                    ]),
                },
                KirElement {
                    id: "flow.Demo.f".to_string(),
                    kind: "Core::Kernel::Flow".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "connectorEnd".to_string(),
                        json!(["type.Demo.flowEnd1", "type.Demo.flowEnd2"]),
                    )]),
                },
                KirElement {
                    id: "feature.Demo.payload".to_string(),
                    kind: "Core::Kernel::PayloadFeature".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("flow.Demo.f")),
                        (
                            "specializes".to_string(),
                            json!(["Core::Kernel::Flow::payloadFeature"]),
                        ),
                        ("type".to_string(), json!("type.Demo.A")),
                    ]),
                },
                KirElement {
                    id: "type.Demo.flowEnd1".to_string(),
                    kind: "Core::Core::Type".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "ownedFeature".to_string(),
                        json!(["feature.Demo.source"]),
                    )]),
                },
                KirElement {
                    id: "type.Demo.flowEnd2".to_string(),
                    kind: "Core::Core::Type".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "ownedFeature".to_string(),
                        json!(["feature.Demo.targetInput"]),
                    )]),
                },
                KirElement {
                    id: "feature.Demo.source".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "feature.Demo.targetInput".to_string(),
                    kind: "Core::Core::Feature".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "note.Demo.annotating".to_string(),
                    kind: "Core::Root::AnnotatingElement".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "ownedAnnotatingRelationship".to_string(),
                        json!("annotation.Demo.note"),
                    )]),
                },
                KirElement {
                    id: "annotation.Demo.note".to_string(),
                    kind: "Core::Root::Annotation".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "req.Demo.R1".to_string(),
                    kind: "Model::Systems::RequirementDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "doc.req.Demo.R1".to_string(),
                    kind: "Core::Root::Documentation".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("req.Demo.R1")),
                        ("body".to_string(), json!("shall brake")),
                    ]),
                },
                KirElement {
                    id: "usage.Demo.u".to_string(),
                    kind: "Model::Systems::Usage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([("isComposite".to_string(), json!(false))]),
                },
            ],
        })
        .unwrap();
        let registry = DerivedFeatureRegistry::with_builtin_core_specs();
        let cache = DerivedFeatureCache::new("revision-12");

        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("type.Demo.A").unwrap(),
                    "owningNamespace",
                )
                .map(|value| value.value),
            Some(Value::String("pkg.Demo".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("pkg.Demo").unwrap(),
                    "membership",
                )
                .map(|value| value.value),
            Some(Value::String("member.Demo.a".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("member.Demo.a").unwrap(),
                    "memberElementId",
                )
                .map(|value| value.value),
            Some(Value::String("type.Demo.A".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("rel.Demo.r").unwrap(),
                    "relatedElement",
                )
                .map(|value| value.value),
            Some(json!(["type.Demo.A", "type.Demo.B"]))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("import.Demo.B").unwrap(),
                    "importedElement",
                )
                .map(|value| value.value),
            Some(Value::String("type.Demo.B".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("type.Demo.T").unwrap(),
                    "unioningType",
                )
                .map(|value| value.value),
            Some(Value::String("type.Demo.A".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("type.Demo.T").unwrap(),
                    "intersectingType",
                )
                .map(|value| value.value),
            Some(Value::String("type.Demo.B".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("type.Demo.T").unwrap(),
                    "differencingType",
                )
                .map(|value| value.value),
            Some(Value::String("type.Demo.B".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("type.Demo.T").unwrap(),
                    "featureMembership",
                )
                .map(|value| value.value),
            Some(Value::String("member.Demo.T.feature".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("type.Demo.T").unwrap(),
                    "isConjugated",
                )
                .map(|value| value.value),
            Some(Value::Bool(true))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("feature.Demo.chain").unwrap(),
                    "chainingFeature",
                )
                .map(|value| value.value),
            Some(Value::String("feature.Demo.target".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("feature.Demo.chain").unwrap(),
                    "featureTarget",
                )
                .map(|value| value.value),
            Some(Value::String("feature.Demo.target".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph
                        .element_by_element_id("feature.Demo.crossing")
                        .unwrap(),
                    "crossFeature",
                )
                .map(|value| value.value),
            Some(Value::String("feature.Demo.targetInput".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph
                        .element_by_element_id("feature.Demo.withTypeFeaturing")
                        .unwrap(),
                    "featuringType",
                )
                .map(|value| value.value),
            Some(Value::String("type.Demo.T".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("flow.Demo.f").unwrap(),
                    "payloadType",
                )
                .map(|value| value.value),
            Some(Value::String("type.Demo.A".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("flow.Demo.f").unwrap(),
                    "sourceOutputFeature",
                )
                .map(|value| value.value),
            Some(Value::String("feature.Demo.source".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("flow.Demo.f").unwrap(),
                    "targetInputFeature",
                )
                .map(|value| value.value),
            Some(Value::String("feature.Demo.targetInput".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("note.Demo.annotating").unwrap(),
                    "annotation",
                )
                .map(|value| value.value),
            Some(Value::String("annotation.Demo.note".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("req.Demo.R1").unwrap(),
                    "text",
                )
                .map(|value| value.value),
            Some(Value::String("shall brake".to_string()))
        );
        assert_eq!(
            cache
                .derived_property(
                    &registry,
                    &graph,
                    graph.element_by_element_id("usage.Demo.u").unwrap(),
                    "isReference",
                )
                .map(|value| value.value),
            Some(Value::Bool(true))
        );
    }

    #[test]
    fn manifest_native_rule_invokes_named_core_function() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.Demo.A".to_string(),
                kind: "Model::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([("declared_name".to_string(), json!("A"))]),
            }],
        })
        .unwrap();
        let registry =
            DerivedFeatureRegistry::with_manifest_and_builtins(Some(DerivedFeatureManifest {
                metamodel: Some("test".to_string()),
                derived_features: vec![DerivedFeatureSpec {
                    owner: "*".to_string(),
                    feature: "nativeName".to_string(),
                    rule: DerivedFeatureRule::Native {
                        function: "core.name".to_string(),
                    },
                }],
            }))
            .unwrap();
        let cache = DerivedFeatureCache::new("revision-9");
        let element = graph.element_by_element_id("type.Demo.A").unwrap();

        assert_eq!(
            cache
                .derived_property(&registry, &graph, element, "nativeName")
                .map(|value| value.value),
            Some(Value::String("A".to_string()))
        );
    }

    #[test]
    fn manifest_rejects_unknown_native_function() {
        let metadata = BTreeMap::from([(
            "derived_feature_manifest".to_string(),
            json!({
                "metamodel": "test",
                "derived_features": [
                    {
                        "owner": "*",
                        "feature": "bad",
                        "kind": "native",
                        "function": "package.inline_script"
                    }
                ]
            }),
        )]);

        assert_eq!(
            manifest_from_metadata(&metadata).unwrap_err(),
            DerivedFeatureManifestError::UnknownNativeFunction("package.inline_script".to_string())
        );
    }
}
