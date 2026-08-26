use std::collections::BTreeMap;

use crate::language_contracts::{DocumentSymbols, SymbolDescriptor};

#[derive(Debug, Clone, Default)]
pub struct WorkspaceSymbolIndex {
    documents: BTreeMap<String, DocumentSymbols>,
    update_counts: BTreeMap<String, u64>,
}

impl WorkspaceSymbolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, document: DocumentSymbols) -> bool {
        if self
            .documents
            .get(&document.source_name)
            .is_some_and(|existing| existing.revision == document.revision)
        {
            return false;
        }
        let source_name = document.source_name.clone();
        self.documents.insert(source_name.clone(), document);
        *self.update_counts.entry(source_name).or_default() += 1;
        true
    }

    pub fn remove(&mut self, source_name: &str) -> Option<DocumentSymbols> {
        self.documents.remove(source_name)
    }

    pub fn document(&self, source_name: &str) -> Option<&DocumentSymbols> {
        self.documents.get(source_name)
    }

    pub fn all_symbols(&self) -> Vec<SymbolDescriptor> {
        self.documents
            .values()
            .flat_map(|document| document.symbols.iter().cloned())
            .collect()
    }

    pub fn symbols_by_qualified_name_prefix(&self, prefix: &str) -> Vec<SymbolDescriptor> {
        let mut symbols = self
            .all_symbols()
            .into_iter()
            .filter(|symbol| symbol.qualified_name.starts_with(prefix))
            .collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            left.qualified_name
                .cmp(&right.qualified_name)
                .then_with(|| left.element_id.cmp(&right.element_id))
        });
        symbols
    }

    pub fn symbol_by_element_id(&self, element_id: &str) -> Option<SymbolDescriptor> {
        self.documents
            .values()
            .flat_map(|document| document.symbols.iter())
            .find(|symbol| symbol.element_id == element_id)
            .cloned()
    }

    pub fn update_count(&self, source_name: &str) -> u64 {
        self.update_counts
            .get(source_name)
            .copied()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crate::language_contracts::{Concept, DocumentSymbols, SymbolDescriptor, TextRange};

    use super::WorkspaceSymbolIndex;

    #[test]
    fn revision_updates_only_the_changed_document() {
        let mut index = WorkspaceSymbolIndex::new();
        assert!(index.update(document("a.sysml", 1, "Demo.A")));
        assert!(index.update(document("b.sysml", 1, "Demo.B")));
        assert!(!index.update(document("a.sysml", 1, "Demo.A")));
        assert!(index.update(document("a.sysml", 2, "Demo.A2")));
        assert_eq!(index.update_count("a.sysml"), 2);
        assert_eq!(index.update_count("b.sysml"), 1);
        assert_eq!(index.symbols_by_qualified_name_prefix("Demo.").len(), 2);
    }

    fn document(source_name: &str, revision: u64, qualified_name: &str) -> DocumentSymbols {
        DocumentSymbols {
            source_name: source_name.to_string(),
            revision,
            symbols: vec![SymbolDescriptor {
                qualified_name: qualified_name.to_string(),
                concept: Concept::PART_DEFINITION,
                span: TextRange::new(0, 4),
                element_id: format!("type.{qualified_name}"),
                source_name: source_name.to_string(),
            }],
        }
    }
}
