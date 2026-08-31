use std::collections::{BTreeMap, BTreeSet};

use crate::codegen::language::{Concept, LanguageProfile};
use crate::kir::{KirDocument, KirElement};
use crate::model::{Graph, MetamodelFeatureRegistry, MetamodelFeatureView};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonWrapperGeneration {
    pub module_name: String,
    pub profile_id: String,
    pub stdlib_version: String,
    pub kir_schema_version: String,
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonFacadeGeneration {
    pub module_name: String,
    pub files: BTreeMap<String, String>,
}

pub fn generate_python_facades(
    document: &KirDocument,
    module_name: &str,
) -> PythonFacadeGeneration {
    let module_path = module_name.replace('.', "/");
    let mut files = BTreeMap::new();
    files.insert(format!("{module_path}/__init__.py"), facade_init_py());
    files.insert(format!("{module_path}/base.py"), base_py());
    files.insert(
        format!("{module_path}/facades.py"),
        facades_py(document, None),
    );
    files.insert(
        format!("{module_path}/metamodel.py"),
        metamodel_compatibility_py(),
    );
    files.insert(format!("{module_path}/py.typed"), String::new());
    PythonFacadeGeneration {
        module_name: module_name.to_string(),
        files,
    }
}

pub fn generate_python_wrappers(
    document: &KirDocument,
    profile: &LanguageProfile,
    module_name: &str,
) -> PythonWrapperGeneration {
    let module_path = module_name.replace('.', "/");
    let mut files = generate_python_facades(document, module_name).files;
    files.insert(
        format!("{module_path}/__init__.py"),
        init_py(module_name, profile),
    );
    files.insert(
        format!("{module_path}/facades.py"),
        facades_py(document, Some(profile)),
    );
    files.insert(format!("{module_path}/concepts.py"), concepts_py(profile));
    files.insert(
        format!("{module_path}/generation_info.py"),
        generation_info_py(profile),
    );
    files.insert(
        format!("{module_path}/stdlib/__init__.py"),
        stdlib_init_py(),
    );
    files.insert(
        format!("{module_path}/stdlib/si.py"),
        catalog_py(document, "SI"),
    );
    files.insert(
        format!("{module_path}/stdlib/isq.py"),
        catalog_prefix_py(document, "ISQ"),
    );

    PythonWrapperGeneration {
        module_name: module_name.to_string(),
        profile_id: profile.id.clone(),
        stdlib_version: profile.stdlib_version.clone(),
        kir_schema_version: profile.kir_schema_version.clone(),
        files,
    }
}
pub fn generate_rust_stdlib_consts(document: &KirDocument, metamodel_id: &str) -> String {
    let mut output = format!("// AUTO-GENERATED from {metamodel_id} - do not edit\n\n");
    output.push_str(&rust_catalog_module(document, "isq", "ISQ"));
    output.push('\n');
    output.push_str(&rust_catalog_module(document, "si", "SI"));
    output.push('\n');
    output.push_str(&rust_catalog_module(
        document,
        "scalar_values",
        "ScalarValues",
    ));
    output
}

fn rust_catalog_module(document: &KirDocument, module_name: &str, prefix: &str) -> String {
    let mut entries = BTreeMap::new();
    for element in &document.elements {
        let qualified_name = stdlib_qualified_name(element);
        let Some(qualified_name) = qualified_name.as_deref() else {
            continue;
        };
        if qualified_name == prefix || !qualified_name.starts_with(&format!("{prefix}::")) {
            continue;
        }
        let Some(leaf) = qualified_name.rsplit("::").next() else {
            continue;
        };
        entries.insert(rust_identifier(leaf), qualified_name.to_string());
    }

    let mut output = format!("pub mod {module_name} {{\n");
    output.push_str("    use crate::builder::StdlibRef;\n");
    output.push_str("    use mercurio_foundation::QualifiedName;\n\n");
    if entries.is_empty() {
        output.push_str("    // No entries found in bundled stdlib.\n");
    }
    for (name, qualified_name) in entries {
        output.push_str(&format!(
            "    pub fn {name}() -> StdlibRef {{ StdlibRef(QualifiedName::parse({qualified_name:?})) }}\n"
        ));
    }
    output.push_str("}\n");
    output
}

fn stdlib_qualified_name(element: &KirElement) -> Option<String> {
    if element.id.contains("::") {
        return Some(element.id.clone());
    }
    element
        .properties
        .get("qualified_name")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn facade_init_py() -> String {
    r#""""Generated typed Mercurio metamodel facades."""

from .base import ElementFacade, ElementView
from .facades import (
    METAMODEL_CLASSES,
    METAMODEL_CLASS_BY_KIND,
    METAMODEL_CLASS_BY_METATYPE,
    class_for,
    facade,
    wrap,
)

__all__ = [
    "ElementFacade",
    "ElementView",
    "METAMODEL_CLASSES",
    "METAMODEL_CLASS_BY_KIND",
    "METAMODEL_CLASS_BY_METATYPE",
    "class_for",
    "facade",
    "wrap",
]

for _cls in METAMODEL_CLASSES:
    globals().setdefault(_cls.__name__, _cls)
    if _cls.__name__ not in __all__:
        __all__.append(_cls.__name__)
if METAMODEL_CLASSES:
    del _cls
"#
    .to_string()
}

fn metamodel_compatibility_py() -> String {
    r#""""Compatibility module; generated facades now live in facades.py."""

from .facades import *  # noqa: F401,F403
"#
    .to_string()
}

fn init_py(module_name: &str, profile: &LanguageProfile) -> String {
    format!(
        r#""""Generated Mercurio wrappers for {profile_id}."""

from .base import ElementFacade, ElementView, StdlibRef
from .facades import (
    METAMODEL_CLASSES,
    METAMODEL_CLASS_BY_KIND,
    METAMODEL_CLASS_BY_METATYPE,
    class_for,
    facade,
    wrap,
)
from .concepts import (
    AttributeUsage,
    ConstraintUsage,
    MetadataUsage,
    Package,
    PartDefinition,
    PartUsage,
    RequirementUsage,
    Model,
    VerificationCaseUsage,
)
from .generation_info import KIR_SCHEMA_VERSION, PROFILE_ID, STDLIB_VERSION

__all__ = [
    "AttributeUsage",
    "ConstraintUsage",
    "ElementFacade",
    "ElementView",
    "KIR_SCHEMA_VERSION",
    "METAMODEL_CLASSES",
    "METAMODEL_CLASS_BY_KIND",
    "METAMODEL_CLASS_BY_METATYPE",
    "MetadataUsage",
    "StdlibRef",
    "Package",
    "PartDefinition",
    "PartUsage",
    "PROFILE_ID",
    "RequirementUsage",
    "STDLIB_VERSION",
    "Model",
    "VerificationCaseUsage",
    "class_for",
    "facade",
    "register",
    "wrap",
]

for _cls in METAMODEL_CLASSES:
    globals().setdefault(_cls.__name__, _cls)
    if _cls.__name__ not in __all__:
        __all__.append(_cls.__name__)
del _cls


def register(registry):
    registry.register_profile("{profile_id}", "{module_name}")
    registry.register("package", Package)
    registry.register("part_definition", PartDefinition)
    registry.register("part_usage", PartUsage)
    registry.register("attribute_usage", AttributeUsage)
    registry.register("requirement_usage", RequirementUsage)
    registry.register("verification_case_usage", VerificationCaseUsage)
    registry.register("constraint_usage", ConstraintUsage)
    registry.register("metadata_usage", MetadataUsage)
    register_metamodel = getattr(registry, "register_metamodel_classes", None)
    if callable(register_metamodel):
        register_metamodel(METAMODEL_CLASSES)
    register_metatype = getattr(registry, "register_metatype", None)
    if callable(register_metatype):
        for metatype_id, cls in METAMODEL_CLASS_BY_METATYPE.items():
            register_metatype(metatype_id, cls)
    register_kind = getattr(registry, "register_kind", None)
    if callable(register_kind):
        for kind, cls in METAMODEL_CLASS_BY_KIND.items():
            register_kind(kind, cls)
"#,
        profile_id = profile.id,
        module_name = module_name,
    )
}

fn generation_info_py(profile: &LanguageProfile) -> String {
    format!(
        r#""""Version information for generated Mercurio wrappers."""

PROFILE_ID = {profile_id:?}
STDLIB_VERSION = {stdlib_version:?}
KIR_SCHEMA_VERSION = {kir_schema_version:?}
LANGUAGE_VERSION = {language_version:?}
METAMODEL_VERSION = {metamodel_version:?}
"#,
        profile_id = profile.id,
        stdlib_version = profile.stdlib_version,
        kir_schema_version = profile.kir_schema_version,
        language_version = profile.language_version,
        metamodel_version = profile.metamodel_version,
    )
}

fn base_py() -> String {
    r#"from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, ClassVar


class ElementFacade:
    concept: ClassVar[str | None] = None
    metatype_id: ClassVar[str | None] = None
    metatype_ids: ClassVar[tuple[str, ...]] = ()
    kind_name: ClassVar[str | None] = None
    kind_names: ClassVar[tuple[str, ...]] = ()

    def __init__(self, element: Any):
        self._element = element

    @classmethod
    def wrap(cls, element: Any):
        if isinstance(element, cls):
            return element
        return cls(element)

    @classmethod
    def matches(cls, element: Any) -> bool:
        metatype_id = getattr(element, "metatype_id", None)
        if callable(metatype_id):
            metatype_id = metatype_id()
        kind = getattr(element, "kind", None)
        if callable(kind):
            kind = kind()
        metatype_ids = cls.metatype_ids or ((cls.metatype_id,) if cls.metatype_id else ())
        kind_names = cls.kind_names or ((cls.kind_name,) if cls.kind_name else ())
        return (
            bool(metatype_id and metatype_id in metatype_ids)
            or bool(kind and kind in kind_names)
            or (not metatype_ids and not kind_names and cls.metatype_id is None)
        )

    @property
    def raw(self) -> Any:
        return self._element

    @property
    def id(self) -> str:
        value = getattr(self._element, "id", None)
        if value is None:
            value = getattr(self._element, "qualified_name", None)
        return str(value() if callable(value) else value)

    @property
    def kind(self) -> str:
        value = getattr(self._element, "kind", None)
        return str(value() if callable(value) else value)

    def get(self, name: str) -> Any:
        get = getattr(self._element, "get", None)
        if callable(get):
            return get(name)
        get_json = getattr(self._element, "get_json", None)
        if callable(get_json):
            value = get_json(name)
            return json.loads(value) if value is not None else None
        get_str = getattr(self._element, "get_str", None)
        if callable(get_str):
            return get_str(name)
        return None

    def effective(self, name: str) -> Any:
        effective = getattr(self._element, "effective", None)
        if callable(effective):
            return effective(name)
        effective_json = getattr(self._element, "effective_json", None)
        if callable(effective_json):
            value = effective_json(name)
            return json.loads(value) if value is not None else None
        effective_str = getattr(self._element, "effective_str", None)
        if callable(effective_str):
            return effective_str(name)
        return self.get(name)

    def references(self, name: str) -> list[Any]:
        references = getattr(self._element, "references", None)
        if callable(references):
            return list(references(name))
        value = self.get(name)
        values = value if isinstance(value, (list, tuple)) else (() if value is None else (value,))
        model = getattr(self._element, "_model", None)
        resolve = getattr(model, "resolve", None)
        if not callable(resolve):
            return list(values)
        resolved = []
        for item in values:
            try:
                resolved.append(resolve(item))
            except (KeyError, TypeError, ValueError):
                resolved.append(item)
        return resolved

    def values(self, name: str) -> list[Any]:
        value = self.effective(name)
        if value is None:
            return []
        if isinstance(value, (list, tuple)):
            return list(value)
        return [value]

    def effective_bool(self, name: str) -> bool | None:
        value = self.effective(name)
        return value if isinstance(value, bool) else None

    def effective_int(self, name: str) -> int | None:
        value = self.effective(name)
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    def effective_float(self, name: str) -> float | None:
        value = self.effective(name)
        return float(value) if isinstance(value, (int, float)) and not isinstance(value, bool) else None

    def metadata(self) -> Any:
        metadata = getattr(self._element, "metadata", None)
        return metadata() if callable(metadata) else metadata

    def metadata_by_type(self, type_name: str) -> list[Any]:
        metadata_by_type = getattr(self._element, "metadata_by_type", None)
        if callable(metadata_by_type):
            return metadata_by_type(type_name)
        metadata = self.metadata() or []
        return [
            item
            for item in metadata
            if getattr(item, "type_name", None) == type_name
            or getattr(item, "type", None) == type_name
        ]

    def effective_str(self, name: str) -> str | None:
        value = self.effective(name)
        return value if isinstance(value, str) else None

    @property
    def name(self) -> str | None:
        return self.effective_str("name")

    @property
    def declared_name(self) -> str | None:
        return self.effective_str("declared_name")

    @property
    def qualified_name(self) -> str | None:
        return self.effective_str("qualified_name")

    @property
    def documentation(self) -> str | None:
        return self.effective_str("documentation") or self.effective_str("doc")

    def members(self) -> list[Any]:
        return self.references("members")

    def features(self) -> list[Any]:
        return self.references("features")

    def owner(self) -> list[Any]:
        return self.references("owner")

    def specializes(self) -> list[Any]:
        return self.references("specializes")


# Compatibility alias for wrapper packages generated before facades.py became canonical.
ElementView = ElementFacade


@dataclass(frozen=True)
class StdlibRef:
    id: str

    def bind(self, model: Any) -> Any:
        return model.element(self.id)
"#
    .to_string()
}

#[derive(Debug, Clone, Default)]
struct PythonMetamodelClass {
    class_name: String,
    base_class_names: BTreeSet<String>,
    metatype_ids: BTreeSet<String>,
    kind_names: BTreeSet<String>,
    feature_owner_ids: BTreeSet<String>,
}

fn facades_py(document: &KirDocument, profile: Option<&LanguageProfile>) -> String {
    let classes = metamodel_classes(document, profile);
    let registry = Graph::from_document(document.clone())
        .ok()
        .map(|graph| MetamodelFeatureRegistry::build(&graph));
    let mut output = String::from(
        r#"from __future__ import annotations

from typing import Any

from .base import ElementFacade, ElementView


"#,
    );

    for class in ordered_metamodel_classes(&classes) {
        let bases = if class.base_class_names.is_empty() {
            "ElementFacade".to_string()
        } else {
            class
                .base_class_names
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        output.push_str(&format!("class {}({bases}):\n", class.class_name));
        output.push_str("    concept = None\n");
        output.push_str(&format!(
            "    metatype_id = {}\n",
            python_string_literal(class.metatype_ids.first().map(String::as_str))
        ));
        output.push_str(&format!(
            "    metatype_ids = {}\n",
            python_tuple_literal(&class.metatype_ids)
        ));
        output.push_str(&format!(
            "    kind_name = {}\n",
            python_string_literal(class.kind_names.first().map(String::as_str))
        ));
        output.push_str(&format!(
            "    kind_names = {}\n\n",
            python_tuple_literal(&class.kind_names)
        ));
        if let Some(registry) = registry.as_ref() {
            let mut features = BTreeMap::new();
            for owner_id in &class.feature_owner_ids {
                for feature in registry.declared_features_for(owner_id) {
                    features
                        .entry(feature.kir_property.clone())
                        .or_insert(feature);
                }
            }
            for feature in features.values() {
                emit_python_feature_accessor(&mut output, feature);
            }
        }
    }

    output.push_str("METAMODEL_CLASSES = (\n");
    for class in ordered_metamodel_classes(&classes) {
        output.push_str(&format!("    {},\n", class.class_name));
    }
    output.push_str(")\n\n");

    output.push_str("METAMODEL_CLASS_BY_METATYPE = {\n");
    for class in ordered_metamodel_classes(&classes) {
        for metatype_id in &class.metatype_ids {
            output.push_str(&format!("    {metatype_id:?}: {},\n", class.class_name));
        }
    }
    output.push_str("}\n\n");

    output.push_str("METAMODEL_CLASS_BY_KIND = {\n");
    for class in ordered_metamodel_classes(&classes) {
        for kind_name in &class.kind_names {
            output.push_str(&format!("    {kind_name:?}: {},\n", class.class_name));
        }
    }
    output.push_str(
        r#"}


def _call_or_value(element: Any, name: str):
    value = getattr(element, name, None)
    return value() if callable(value) else value


def class_for(element: Any) -> type[ElementFacade]:
    metatype_id = _call_or_value(element, "metatype_id")
    if metatype_id in METAMODEL_CLASS_BY_METATYPE:
        return METAMODEL_CLASS_BY_METATYPE[metatype_id]
    kind = _call_or_value(element, "kind")
    if kind in METAMODEL_CLASS_BY_KIND:
        return METAMODEL_CLASS_BY_KIND[kind]
    return ElementFacade


def facade(element: Any) -> ElementFacade:
    return class_for(element).wrap(element)


def wrap(element: Any) -> ElementFacade:
    """Compatibility alias for facade()."""
    return facade(element)
"#,
    );
    output
}

fn emit_python_feature_accessor(output: &mut String, feature: &MetamodelFeatureView) {
    let property = feature.kir_property.as_str();
    let method = python_identifier(property);
    if [
        "effective",
        "effective_bool",
        "effective_float",
        "effective_int",
        "effective_str",
        "features",
        "get",
        "id",
        "kind",
        "matches",
        "members",
        "metadata",
        "metadata_by_type",
        "owner",
        "raw",
        "references",
        "specializes",
        "values",
        "wrap",
    ]
    .contains(&method.as_str())
    {
        return;
    }
    let many = feature.multiplicity_upper.as_deref() == Some("*")
        || feature
            .multiplicity_upper
            .as_deref()
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|upper| upper > 1);
    let type_label = feature.type_label.as_deref().unwrap_or_default();
    output.push_str("    @property\n");
    if many {
        output.push_str(&format!(
            "    def {method}(self) -> list[Any]:\n        return self.values({property:?})\n\n"
        ));
    } else if type_label.contains("Boolean") {
        output.push_str(&format!(
            "    def {method}(self) -> bool | None:\n        return self.effective_bool({property:?})\n\n"
        ));
    } else if type_label.contains("Integer") {
        output.push_str(&format!(
            "    def {method}(self) -> int | None:\n        return self.effective_int({property:?})\n\n"
        ));
    } else if type_label.contains("Real") || type_label.contains("Number") {
        output.push_str(&format!(
            "    def {method}(self) -> float | None:\n        return self.effective_float({property:?})\n\n"
        ));
    } else {
        output.push_str(&format!(
            "    def {method}(self) -> str | None:\n        return self.effective_str({property:?})\n\n"
        ));
    }
}

fn metamodel_classes(
    document: &KirDocument,
    profile: Option<&LanguageProfile>,
) -> BTreeMap<String, PythonMetamodelClass> {
    let mut classes = BTreeMap::<String, PythonMetamodelClass>::new();

    for element in &document.elements {
        if matches!(element.kind.as_str(), "Metaclass" | "MetadataDefinition") {
            let class_name = metamodel_class_name_for_element(element);
            let class = classes
                .entry(class_name.clone())
                .or_insert_with(|| PythonMetamodelClass {
                    class_name,
                    ..Default::default()
                });
            class.metatype_ids.insert(element.id.clone());
            class.kind_names.insert(
                metadata_name(element)
                    .or_else(|| element.id.rsplit("::").next())
                    .unwrap_or(element.id.as_str())
                    .to_string(),
            );
            class.feature_owner_ids.insert(element.id.clone());
        }
    }

    let id_to_class_name = document
        .elements
        .iter()
        .filter(|element| matches!(element.kind.as_str(), "Metaclass" | "MetadataDefinition"))
        .map(|element| {
            (
                element.id.as_str(),
                metamodel_class_name_for_element(element),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for element in &document.elements {
        if !matches!(element.kind.as_str(), "Metaclass" | "MetadataDefinition") {
            continue;
        }
        let class_name = metamodel_class_name_for_element(element);
        let base_class_names = specialization_ids(element)
            .into_iter()
            .filter_map(|base_id| id_to_class_name.get(base_id).cloned())
            .filter(|base_class_name| base_class_name != &class_name)
            .collect::<BTreeSet<_>>();
        if !base_class_names.is_empty() {
            classes
                .entry(class_name.clone())
                .or_insert_with(|| PythonMetamodelClass {
                    class_name,
                    ..Default::default()
                })
                .base_class_names
                .extend(base_class_names);
        }
    }

    for kind in document
        .elements
        .iter()
        .map(|element| element.kind.as_str())
        .collect::<BTreeSet<_>>()
    {
        let class_name = python_class_identifier(kind);
        let class = classes
            .entry(class_name.clone())
            .or_insert_with(|| PythonMetamodelClass {
                class_name,
                ..Default::default()
            });
        class.kind_names.insert(kind.to_string());
    }

    if let Some(profile) = profile {
        for target in profile
            .canonical_kinds
            .values()
            .chain(profile.aliases.values())
            .collect::<BTreeSet<_>>()
        {
            let class_name = python_class_identifier(target.rsplit("::").next().unwrap_or(target));
            let class = classes
                .entry(class_name.clone())
                .or_insert_with(|| PythonMetamodelClass {
                    class_name,
                    ..Default::default()
                });
            class.metatype_ids.insert(target.clone());
        }
    }

    remove_redundant_bases(&mut classes);
    classes
}

fn ordered_metamodel_classes(
    classes: &BTreeMap<String, PythonMetamodelClass>,
) -> Vec<&PythonMetamodelClass> {
    let mut ordered = Vec::new();
    let mut temporary = BTreeSet::new();
    let mut permanent = BTreeSet::new();
    for class_name in classes.keys() {
        visit_metamodel_class(
            class_name,
            classes,
            &mut temporary,
            &mut permanent,
            &mut ordered,
        );
    }
    ordered
}

fn visit_metamodel_class<'a>(
    class_name: &str,
    classes: &'a BTreeMap<String, PythonMetamodelClass>,
    temporary: &mut BTreeSet<String>,
    permanent: &mut BTreeSet<String>,
    ordered: &mut Vec<&'a PythonMetamodelClass>,
) {
    if permanent.contains(class_name) {
        return;
    }
    if !temporary.insert(class_name.to_string()) {
        return;
    }
    let Some(class) = classes.get(class_name) else {
        return;
    };
    for base_class_name in &class.base_class_names {
        visit_metamodel_class(base_class_name, classes, temporary, permanent, ordered);
    }
    temporary.remove(class_name);
    permanent.insert(class_name.to_string());
    ordered.push(class);
}

fn remove_redundant_bases(classes: &mut BTreeMap<String, PythonMetamodelClass>) {
    let base_map = classes
        .iter()
        .map(|(class_name, class)| (class_name.clone(), class.base_class_names.clone()))
        .collect::<BTreeMap<_, _>>();
    for class in classes.values_mut() {
        let bases = class.base_class_names.iter().cloned().collect::<Vec<_>>();
        for base in &bases {
            if bases.iter().any(|other| {
                other != base && class_reaches_base(other, base, &base_map, &mut BTreeSet::new())
            }) {
                class.base_class_names.remove(base);
            }
        }
    }
}

fn class_reaches_base(
    class_name: &str,
    target_base: &str,
    base_map: &BTreeMap<String, BTreeSet<String>>,
    seen: &mut BTreeSet<String>,
) -> bool {
    if !seen.insert(class_name.to_string()) {
        return false;
    }
    let Some(bases) = base_map.get(class_name) else {
        return false;
    };
    bases.contains(target_base)
        || bases
            .iter()
            .any(|base| class_reaches_base(base, target_base, base_map, seen))
}

fn metamodel_class_name_for_element(element: &KirElement) -> String {
    python_class_identifier(
        metadata_name(element)
            .or_else(|| element.id.rsplit("::").next())
            .unwrap_or(&element.id),
    )
}

fn specialization_ids(element: &KirElement) -> Vec<&str> {
    element
        .properties
        .get("specializes")
        .map(value_string_list)
        .unwrap_or_default()
}

fn value_string_list(value: &serde_json::Value) -> Vec<&str> {
    if let Some(value) = value.as_str() {
        return vec![value];
    }
    value
        .as_array()
        .map(|items| items.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default()
}

fn metadata_name(element: &KirElement) -> Option<&str> {
    element
        .properties
        .get("metadata")
        .and_then(|value| value.get("name"))
        .and_then(serde_json::Value::as_str)
}

fn concepts_py(profile: &LanguageProfile) -> String {
    let package = python_string_literal(concept_anchor(profile, &Concept::PACKAGE));
    let part_definition = python_string_literal(profile_anchor(profile, "part_definition"));
    let part_usage = python_string_literal(profile_anchor(profile, "part_usage"));
    let attribute_usage = python_string_literal(profile_anchor(profile, "attribute_usage"));
    let requirement_usage = python_string_literal(profile_anchor(profile, "requirement_usage"));
    let verification_case_usage =
        python_string_literal(profile_anchor(profile, "verification_case_usage"));
    let constraint_usage = python_string_literal(profile_anchor(profile, "constraint_usage"));
    format!(
        r#"from __future__ import annotations

from . import facades as _metamodel
from .base import ElementFacade, ElementView
from .stdlib.si import SINamespace
from .stdlib.isq import ISQNamespace


class Package(_metamodel.Package):
    concept = "package"
    metatype_id = {package}

    @property
    def qualified_name(self) -> str | None:
        return self.effective_str("qualified_name")

    def owned_members(self) -> list[ElementView]:
        return self.references("members") or self.references("features")


class PartDefinition(_metamodel.PartDefinition):
    concept = "part_definition"
    metatype_id = {part_definition}

    @property
    def name(self) -> str | None:
        return self.effective_str("name")

    @property
    def qualified_name(self) -> str | None:
        return self.effective_str("qualified_name")

    def features(self) -> list[ElementView]:
        return self.references("features")


class PartUsage(_metamodel.PartUsage):
    concept = "part_usage"
    metatype_id = {part_usage}

    @property
    def name(self) -> str | None:
        return self.effective_str("name")

    @property
    def qualified_name(self) -> str | None:
        return self.effective_str("qualified_name")


class AttributeUsage(_metamodel.AttributeUsage):
    concept = "attribute_usage"
    metatype_id = {attribute_usage}

    @property
    def name(self) -> str | None:
        return self.effective_str("name")


class RequirementUsage(_metamodel.RequirementUsage):
    concept = "requirement_usage"
    metatype_id = {requirement_usage}

    @property
    def text(self) -> str | None:
        return self.effective_str("text") or self.effective_str("documentation")


class VerificationCaseUsage(_metamodel.VerificationCaseUsage):
    concept = "verification_case_usage"
    metatype_id = {verification_case_usage}

    @property
    def name(self) -> str | None:
        return self.effective_str("name")


class ConstraintUsage(_metamodel.ConstraintUsage):
    concept = "constraint_usage"
    metatype_id = {constraint_usage}

    @property
    def expression(self):
        return self.effective("expression")


class MetadataUsage(ElementView):
    concept = "metadata_usage"
    metatype_id = None

    @property
    def metadata_type(self) -> str | None:
        return self.effective_str("metadata_type") or self.effective_str("type")


class StdlibNamespace:
    def __init__(self, model):
        self.SI = SINamespace(model)
        self.ISQ = ISQNamespace(model)


class Model:
    def __init__(self, model):
        self.model = model
        self.stdlib = StdlibNamespace(model)

    @classmethod
    def bind(cls, model):
        return cls(model)

    def elements_with_metadata(self, metadata_type: str) -> list[ElementView]:
        query = getattr(self.model, "elements_with_metadata", None)
        if callable(query):
            return query(metadata_type)
        return []
"#
    )
}

fn stdlib_init_py() -> String {
    "from .si import SINamespace\nfrom .isq import ISQNamespace\n".to_string()
}

fn catalog_py(document: &KirDocument, owner: &str) -> String {
    catalog_module_py(
        "SINamespace",
        entries_for_owner(document, owner).into_iter().take(600),
    )
}

fn catalog_prefix_py(document: &KirDocument, prefix: &str) -> String {
    catalog_module_py(
        "ISQNamespace",
        document
            .elements
            .iter()
            .filter(|element| element.id.starts_with(prefix))
            .filter_map(|element| {
                let leaf = element.id.rsplit("::").next()?;
                Some((python_identifier(leaf), element.id.clone()))
            })
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .take(1200),
    )
}

fn catalog_module_py<I>(class_name: &str, entries: I) -> String
where
    I: IntoIterator<Item = (String, String)>,
{
    let entries = entries.into_iter().collect::<Vec<_>>();
    let mut output = String::from("from __future__ import annotations\n\n\n");
    output.push_str(&format!("class {class_name}:\n"));
    output.push_str("    def __init__(self, model):\n");
    output.push_str("        self._model = model\n\n");
    if entries.is_empty() {
        output.push_str("    pass\n");
        return output;
    }
    for (name, id) in entries {
        output.push_str("    @property\n");
        output.push_str(&format!("    def {name}(self):\n"));
        output.push_str(&format!("        return self._model.element({id:?})\n\n"));
    }
    output
}

fn entries_for_owner(document: &KirDocument, owner: &str) -> BTreeMap<String, String> {
    document
        .elements
        .iter()
        .filter(|element| {
            element
                .properties
                .get("owner")
                .and_then(|value| value.as_str())
                == Some(owner)
                || element.id.starts_with(&format!("{owner}::"))
        })
        .filter_map(|element| {
            let leaf = element.id.rsplit("::").next()?;
            Some((python_identifier(leaf), element.id.clone()))
        })
        .collect()
}

fn concept_anchor<'a>(profile: &'a LanguageProfile, concept: &Concept) -> Option<&'a str> {
    profile.canonical_kinds.get(concept).map(String::as_str)
}

fn profile_anchor<'a>(profile: &'a LanguageProfile, concept: &str) -> Option<&'a str> {
    profile.semantic_anchors.get(concept).map(String::as_str)
}

fn python_string_literal(value: Option<&str>) -> String {
    value
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "None".to_string())
}

fn python_tuple_literal(values: &BTreeSet<String>) -> String {
    match values.len() {
        0 => "()".to_string(),
        1 => format!("({:?},)", values.first().expect("one value")),
        _ => format!(
            "({})",
            values
                .iter()
                .map(|value| format!("{value:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn python_class_identifier(value: &str) -> String {
    let identifier = python_identifier(value);
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return "Element".to_string();
    };
    let mut result = first.to_ascii_uppercase().to_string();
    result.push_str(chars.as_str());
    if result.starts_with('_') {
        result.insert_str(0, "Element");
    }
    result
}

fn python_identifier(value: &str) -> String {
    let mut result = String::new();
    for (index, ch) in value.chars().enumerate() {
        let valid = ch == '_' || ch.is_ascii_alphanumeric();
        if index == 0 && ch.is_ascii_digit() {
            result.push('_');
        }
        result.push(if valid { ch } else { '_' });
    }
    while result.contains("__") {
        result = result.replace("__", "_");
    }
    result = result.trim_matches('_').to_string();
    if result.is_empty() {
        result = "element".to_string();
    }
    if result
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        result.insert(0, '_');
    }
    if python_keywords().contains(result.as_str()) {
        result.push('_');
    }
    result
}

fn rust_identifier(value: &str) -> String {
    let mut result = String::new();
    for (index, ch) in value.chars().enumerate() {
        let valid = ch == '_' || ch.is_ascii_alphanumeric();
        if index == 0 && ch.is_ascii_digit() {
            result.push('_');
        }
        result.push(if valid { ch.to_ascii_lowercase() } else { '_' });
    }
    while result.contains("__") {
        result = result.replace("__", "_");
    }
    result = result.trim_matches('_').to_string();
    if result.is_empty() {
        result = "element".to_string();
    }
    if result
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        result.insert(0, '_');
    }
    if rust_keywords().contains(result.as_str()) {
        result.push('_');
    }
    result
}

fn rust_keywords() -> BTreeSet<&'static str> {
    [
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn",
    ]
    .into_iter()
    .collect()
}

fn python_keywords() -> BTreeSet<&'static str> {
    [
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield",
    ]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::codegen::{Concept, LanguageId, LanguageProfile};
    use crate::kir::{KirDocument, KirElement};

    use super::*;

    #[test]
    fn generates_initial_wrapper_files() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "SI::metre".to_string(),
                kind: "AttributeUsage".to_string(),
                layer: 1,
                properties: BTreeMap::new(),
            }],
        };
        let profile = LanguageProfile {
            id: "model-test".to_string(),
            language: LanguageId::from("model"),
            language_version: "2.0".to_string(),
            metamodel_version: "2.0".to_string(),
            stdlib_version: "test".to_string(),
            stdlib_path: "stdlib.kir.json".to_string(),
            kir_schema_version: "0.2".to_string(),
            canonical_kinds: BTreeMap::from([(Concept::PACKAGE, "Model::Package".to_string())]),
            semantic_anchors: BTreeMap::new(),
            aliases: BTreeMap::new(),
        };

        let generated = generate_python_wrappers(&document, &profile, "mercurio_model_test");
        assert_eq!(generated.profile_id, "model-test");
        assert_eq!(generated.stdlib_version, "test");
        assert!(
            generated
                .files
                .contains_key("mercurio_model_test/__init__.py")
        );
        assert!(
            generated
                .files
                .contains_key("mercurio_model_test/generation_info.py")
        );
        assert!(generated.files["mercurio_model_test/stdlib/si.py"].contains("def metre(self)"));
        assert!(
            generated.files["mercurio_model_test/concepts.py"].contains("class PartDefinition")
        );
        assert!(
            generated
                .files
                .contains_key("mercurio_model_test/metamodel.py")
        );
        assert!(
            generated
                .files
                .contains_key("mercurio_model_test/facades.py")
        );
    }

    #[test]
    fn generates_metamodel_classes_for_metaclasses_metadata_definitions_and_kinds() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "Core::Kernel::Package".to_string(),
                    kind: "Metaclass".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "metadata".to_string(),
                        serde_json::json!({"name": "Package"}),
                    )]),
                },
                KirElement {
                    id: "Model::Systems::ItemDefinition".to_string(),
                    kind: "MetadataDefinition".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "metadata".to_string(),
                        serde_json::json!({"name": "ItemDefinition"}),
                    )]),
                },
                KirElement {
                    id: "Model::Systems::PartDefinition".to_string(),
                    kind: "MetadataDefinition".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([
                        (
                            "metadata".to_string(),
                            serde_json::json!({"name": "PartDefinition"}),
                        ),
                        (
                            "specializes".to_string(),
                            serde_json::json!(["Model::Systems::ItemDefinition"]),
                        ),
                    ]),
                },
                KirElement {
                    id: "feature.part_definition.is_abstract".to_string(),
                    kind: "MetamodelFeature".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([
                        (
                            "owner".to_string(),
                            serde_json::json!("Model::Systems::PartDefinition"),
                        ),
                        ("kir_property".to_string(), serde_json::json!("is_abstract")),
                        ("type_label".to_string(), serde_json::json!("Boolean")),
                    ]),
                },
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::new(),
                },
            ],
        };
        let profile = LanguageProfile {
            id: "model-test".to_string(),
            language: LanguageId::from("model"),
            language_version: "2.0".to_string(),
            metamodel_version: "2.0".to_string(),
            stdlib_version: "test".to_string(),
            stdlib_path: "stdlib.kir.json".to_string(),
            kir_schema_version: "0.2".to_string(),
            canonical_kinds: BTreeMap::new(),
            semantic_anchors: BTreeMap::from([(
                "part_definition".to_string(),
                "Model::Systems::PartDefinition".to_string(),
            )]),
            aliases: BTreeMap::from([(
                "Model::PartDefinition".to_string(),
                "Model::Systems::PartDefinition".to_string(),
            )]),
        };

        let generated = generate_python_wrappers(&document, &profile, "mercurio_model_test");
        let metamodel = &generated.files["mercurio_model_test/facades.py"];
        let concepts = &generated.files["mercurio_model_test/concepts.py"];

        assert!(metamodel.contains("class Package(ElementFacade):"));
        assert!(metamodel.contains("class ItemDefinition(ElementFacade):"));
        assert!(metamodel.contains("class PartDefinition(ItemDefinition):"));
        assert!(metamodel.contains(r#""Model::Systems::PartDefinition": PartDefinition"#));
        assert!(metamodel.contains(r#""PartDefinition": PartDefinition"#));
        assert!(metamodel.contains("def class_for(element: Any) -> type[ElementFacade]:"));
        assert!(metamodel.contains("def is_abstract(self) -> bool | None:"));
        assert!(metamodel.contains("return self.effective_bool(\"is_abstract\")"));
        assert!(concepts.contains("class PartDefinition(_metamodel.PartDefinition):"));
    }
}
