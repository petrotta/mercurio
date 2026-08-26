use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use rhai::{Array, Dynamic, Engine, Map};

use super::types::{DslElement, ElementSet, ModelContext};

pub fn register_stdlib(engine: &mut Engine) {
    engine.register_fn("count_by_kind", |set: &mut ElementSet| -> Map {
        let mut counts = BTreeMap::<String, i64>::new();
        for id in &set.ids {
            if let Some(element) = set.graph.element(*id) {
                *counts.entry(element.kind.as_ref().to_string()).or_default() += 1;
            }
        }
        counts
            .into_iter()
            .map(|(kind, count)| (kind.into(), Dynamic::from(count)))
            .collect()
    });

    engine.register_fn("sum", |values: Array| -> f64 {
        values.into_iter().filter_map(dynamic_to_f64).sum()
    });

    engine.register_fn("max", |values: Array| -> Dynamic {
        numeric_extreme(values, f64::max)
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT)
    });
    engine.register_fn("max", |left: Dynamic, right: Dynamic| -> Dynamic {
        binary_numeric_extreme(left, right, f64::max)
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT)
    });
    engine.register_fn("min", |values: Array| -> Dynamic {
        numeric_extreme(values, f64::min)
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT)
    });
    engine.register_fn("min", |left: Dynamic, right: Dynamic| -> Dynamic {
        binary_numeric_extreme(left, right, f64::min)
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT)
    });

    engine.register_fn("all_parts", |context: &mut ModelContext| -> ElementSet {
        context.parts()
    });

    engine.register_fn(
        "reachable",
        |element: &mut DslElement, relation: String| -> ElementSet {
            let graph = Arc::clone(&element.graph);
            let mut visited = BTreeSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(element.id);

            while let Some(id) = queue.pop_front() {
                if !visited.insert(id) {
                    continue;
                }
                for edge in graph.outgoing(id, &relation) {
                    queue.push_back(edge.target);
                }
            }

            visited.remove(&element.id);
            ElementSet {
                ids: visited.into_iter().collect(),
                graph,
            }
        },
    );

    engine.register_fn("specialization_depth", |element: &mut DslElement| -> i64 {
        let graph = Arc::clone(&element.graph);
        let mut depth = 0i64;
        let mut current = element.id;
        let mut visited = BTreeSet::new();

        loop {
            if !visited.insert(current) {
                break;
            }
            match graph
                .outgoing(current, "specializes")
                .next()
                .map(|edge| edge.target)
            {
                Some(parent) => {
                    depth += 1;
                    current = parent;
                }
                None => break,
            }
        }

        depth
    });
}

fn dynamic_to_f64(value: Dynamic) -> Option<f64> {
    if value.is::<i64>() {
        return Some(value.cast::<i64>() as f64);
    }
    if value.is::<f64>() {
        return Some(value.cast::<f64>());
    }
    if value.is::<String>() {
        return value.cast::<String>().parse().ok();
    }
    None
}

fn numeric_extreme(values: Array, reducer: fn(f64, f64) -> f64) -> Option<f64> {
    values
        .into_iter()
        .filter_map(dynamic_to_f64)
        .reduce(reducer)
}

fn binary_numeric_extreme(
    left: Dynamic,
    right: Dynamic,
    reducer: fn(f64, f64) -> f64,
) -> Option<f64> {
    Some(reducer(dynamic_to_f64(left)?, dynamic_to_f64(right)?))
}
