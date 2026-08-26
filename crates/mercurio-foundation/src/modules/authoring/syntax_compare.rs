use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::authoring::frontend::ast::{
    AliasDecl, Declaration, Expr, GenericDefinitionDecl, GenericUsageDecl, ImportDecl, PackageDecl,
    ParsedModule, QualifiedName, SourceSpan,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxSnapshot {
    pub root_kind: String,
    pub nodes: Vec<SyntaxSnapshotNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxSnapshotNode {
    pub path: String,
    pub family: String,
    pub kind: String,
    pub keyword: String,
    pub declared_name: Option<String>,
    pub span: SyntaxSourceSpan,
    #[serde(default)]
    pub properties: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SyntaxSourceSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxComparisonReport {
    pub rust_count: usize,
    pub pilot_count: usize,
    pub exact_match_count: usize,
    pub mismatches: Vec<SyntaxNodeMismatch>,
    pub rust_only: Vec<SyntaxSnapshotNode>,
    pub pilot_only: Vec<SyntaxSnapshotNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxNodeMismatch {
    pub span: SyntaxSourceSpan,
    pub rust_node: SyntaxSnapshotNode,
    pub pilot_node: SyntaxSnapshotNode,
    pub differences: Vec<String>,
}

pub fn build_rust_syntax_snapshot(module: &ParsedModule) -> SyntaxSnapshot {
    let mut nodes = Vec::new();
    for (index, declaration) in module.members.iter().enumerate() {
        let path = format!("{index}");
        push_declaration_node(declaration, &path, &mut nodes);
    }
    SyntaxSnapshot {
        root_kind: "ParsedModule".to_string(),
        nodes,
    }
}

pub fn compare_syntax_snapshots(
    rust: SyntaxSnapshot,
    pilot: SyntaxSnapshot,
) -> SyntaxComparisonReport {
    let mut rust_by_span = BTreeMap::new();
    for node in rust.nodes {
        rust_by_span.insert(syntax_match_key(&node), node);
    }

    let mut pilot_by_span = BTreeMap::new();
    for node in pilot.nodes {
        pilot_by_span.insert(syntax_match_key(&node), node);
    }

    let mut exact_match_count = 0;
    let mut mismatches = Vec::new();
    let mut rust_only = Vec::new();
    let mut pilot_only = Vec::new();

    for (key, rust_node) in &rust_by_span {
        match pilot_by_span.get(key) {
            Some(pilot_node) => {
                let differences = compare_nodes(rust_node, pilot_node);
                if differences.is_empty() {
                    exact_match_count += 1;
                } else {
                    mismatches.push(SyntaxNodeMismatch {
                        span: rust_node.span.clone(),
                        rust_node: rust_node.clone(),
                        pilot_node: pilot_node.clone(),
                        differences,
                    });
                }
            }
            None => rust_only.push(rust_node.clone()),
        }
    }

    for (key, pilot_node) in &pilot_by_span {
        if !rust_by_span.contains_key(key) {
            pilot_only.push(pilot_node.clone());
        }
    }

    SyntaxComparisonReport {
        rust_count: rust_by_span.len(),
        pilot_count: pilot_by_span.len(),
        exact_match_count,
        mismatches,
        rust_only,
        pilot_only,
    }
}

fn compare_nodes(rust: &SyntaxSnapshotNode, pilot: &SyntaxSnapshotNode) -> Vec<String> {
    let mut differences = Vec::new();
    if rust.family != pilot.family {
        differences.push(format!(
            "family mismatch: rust={} pilot={}",
            rust.family, pilot.family
        ));
    }
    if rust.keyword != pilot.keyword {
        differences.push(format!(
            "keyword mismatch: rust={} pilot={}",
            rust.keyword, pilot.keyword
        ));
    }
    if rust.declared_name != pilot.declared_name {
        differences.push(format!(
            "declared_name mismatch: rust={:?} pilot={:?}",
            rust.declared_name, pilot.declared_name
        ));
    }

    let property_keys = rust
        .properties
        .keys()
        .filter(|key| pilot.properties.contains_key(*key))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    for key in property_keys {
        let rust_value = rust.properties.get(&key);
        let pilot_value = pilot.properties.get(&key);
        if rust_value != pilot_value {
            differences.push(format!(
                "property `{}` mismatch: rust={:?} pilot={:?}",
                key, rust_value, pilot_value
            ));
        }
    }

    differences
}

fn syntax_match_key(node: &SyntaxSnapshotNode) -> String {
    format!(
        "{}|{}|{}|{}",
        node.span.start_line,
        node.family,
        node.declared_name.clone().unwrap_or_default(),
        node.keyword
    )
}

fn push_declaration_node(
    declaration: &Declaration,
    path: &str,
    nodes: &mut Vec<SyntaxSnapshotNode>,
) {
    if let Some(definition) = declaration.as_definition_like() {
        nodes.push(generic_definition_node(&definition, path));
        for (index, child) in definition.members.iter().enumerate() {
            let child_path = format!("{path}.{index}");
            push_declaration_node(child, &child_path, nodes);
        }
        return;
    }
    if let Some(usage) = declaration.as_usage_like() {
        nodes.push(generic_usage_node(&usage, path));
        for (index, child) in usage.body_members.iter().enumerate() {
            let child_path = format!("{path}.{index}");
            push_declaration_node(child, &child_path, nodes);
        }
        return;
    }

    let node = match declaration {
        Declaration::Package(package) => package_node(package, path),
        Declaration::Import(import) => import_node(import, path),
        Declaration::GenericDefinition(_) | Declaration::GenericUsage(_) => {
            unreachable!("generic declarations are handled by projection helpers")
        }
        Declaration::Alias(alias) => alias_node(alias, path),
    };
    nodes.push(node);

    let children = declaration.child_declarations();

    for (index, child) in children.iter().enumerate() {
        let child_path = format!("{path}.{index}");
        push_declaration_node(child, &child_path, nodes);
    }
}

fn package_node(package: &PackageDecl, path: &str) -> SyntaxSnapshotNode {
    let mut properties = BTreeMap::new();
    insert_list(
        &mut properties,
        "qualified_name",
        [package.name.as_colon_string()],
    );
    insert_list(
        &mut properties,
        "modifiers",
        package.modifiers.iter().cloned(),
    );
    SyntaxSnapshotNode {
        path: path.to_string(),
        family: "package".to_string(),
        kind: "PackageDecl".to_string(),
        keyword: "package".to_string(),
        declared_name: Some(package.name.as_colon_string()),
        span: convert_span(&package.span),
        properties,
    }
}

fn import_node(import: &ImportDecl, path: &str) -> SyntaxSnapshotNode {
    let mut properties = BTreeMap::new();
    insert_list(&mut properties, "path", [import.path.as_colon_string()]);
    insert_list(
        &mut properties,
        "modifiers",
        import.modifiers.iter().cloned(),
    );
    SyntaxSnapshotNode {
        path: path.to_string(),
        family: "import".to_string(),
        kind: "ImportDecl".to_string(),
        keyword: "import".to_string(),
        declared_name: None,
        span: convert_span(&import.span),
        properties,
    }
}

fn alias_node(alias: &AliasDecl, path: &str) -> SyntaxSnapshotNode {
    let mut properties = BTreeMap::new();
    insert_list(&mut properties, "target", [alias.target.as_colon_string()]);
    insert_list(
        &mut properties,
        "modifiers",
        alias.modifiers.iter().cloned(),
    );
    SyntaxSnapshotNode {
        path: path.to_string(),
        family: "alias".to_string(),
        kind: "AliasDecl".to_string(),
        keyword: "alias".to_string(),
        declared_name: Some(alias.name.clone()),
        span: convert_span(&alias.span),
        properties,
    }
}

fn generic_definition_node(definition: &GenericDefinitionDecl, path: &str) -> SyntaxSnapshotNode {
    let mut properties = BTreeMap::new();
    insert_list(
        &mut properties,
        "specializes",
        definition
            .specializes
            .iter()
            .map(QualifiedName::as_colon_string),
    );
    insert_list(
        &mut properties,
        "modifiers",
        definition.modifiers.iter().cloned(),
    );
    SyntaxSnapshotNode {
        path: path.to_string(),
        family: "definition".to_string(),
        kind: "GenericDefinitionDecl".to_string(),
        keyword: definition.keyword.clone(),
        declared_name: Some(definition.name.clone()),
        span: convert_span(&definition.span),
        properties,
    }
}

fn generic_usage_node(usage: &GenericUsageDecl, path: &str) -> SyntaxSnapshotNode {
    let mut properties = usage_properties(usage.ty.as_ref(), usage.expression.as_ref());
    insert_list(
        &mut properties,
        "additional_types",
        usage
            .additional_types
            .iter()
            .map(QualifiedName::as_colon_string),
    );
    insert_list(
        &mut properties,
        "specializes",
        usage.specializes.iter().map(QualifiedName::as_colon_string),
    );
    insert_list(
        &mut properties,
        "subsets",
        usage.subsets.iter().map(QualifiedName::as_colon_string),
    );
    insert_list(
        &mut properties,
        "redefines",
        usage.redefines.iter().map(QualifiedName::as_colon_string),
    );
    insert_list(
        &mut properties,
        "modifiers",
        usage.modifiers.iter().cloned(),
    );
    insert_list(
        &mut properties,
        "implicit_name",
        [usage.is_implicit_name.to_string()],
    );
    if let Some(reference_target) = &usage.reference_target {
        insert_list(
            &mut properties,
            "reference_target",
            [reference_target.as_colon_string()],
        );
    }
    SyntaxSnapshotNode {
        path: path.to_string(),
        family: "usage".to_string(),
        kind: "GenericUsageDecl".to_string(),
        keyword: usage.keyword.clone(),
        declared_name: (!usage.is_implicit_name).then(|| usage.name.clone()),
        span: convert_span(&usage.span),
        properties,
    }
}

fn usage_properties(
    ty: Option<&QualifiedName>,
    expression: Option<&Expr>,
) -> BTreeMap<String, Vec<String>> {
    let mut properties = BTreeMap::new();
    if let Some(ty) = ty {
        insert_list(&mut properties, "type", [ty.as_colon_string()]);
    }
    if let Some(expression) = expression {
        insert_list(
            &mut properties,
            "expression",
            [expr_kind(expression).to_string()],
        );
    }
    properties
}

fn expr_kind(expr: &Expr) -> &'static str {
    match expr {
        Expr::Literal(_) => "literal",
        Expr::Name(_) => "name",
        Expr::SelfRef(_) => "self",
        Expr::Tuple { .. } => "tuple",
        Expr::Unary { .. } => "unary",
        Expr::Binary { .. } => "binary",
        Expr::Path { .. } => "path",
        Expr::Call { .. } => "call",
    }
}

fn insert_list<I>(properties: &mut BTreeMap<String, Vec<String>>, key: &str, values: I)
where
    I: IntoIterator<Item = String>,
{
    let values = values.into_iter().collect::<Vec<_>>();
    if !values.is_empty() {
        properties.insert(key.to_string(), values);
    }
}

fn convert_span(span: &SourceSpan) -> SyntaxSourceSpan {
    SyntaxSourceSpan {
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}
