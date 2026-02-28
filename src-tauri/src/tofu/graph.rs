use std::collections::{HashMap, HashSet, VecDeque};

use regex::Regex;

use crate::tofu::types::{DependencyGraph, GraphEdge, GraphNode, TofuResourceConfig};

/// Build a dependency graph from project resources by scanning field values
/// for references to other resources (e.g. `aws_vpc.main.id`).
pub fn build_dependency_graph(resources: &[TofuResourceConfig]) -> DependencyGraph {
    if resources.is_empty() {
        return DependencyGraph {
            nodes: vec![],
            edges: vec![],
        };
    }

    // Build lookup: "resource_type.logical_name" -> resource id
    let mut lookup: HashMap<String, &TofuResourceConfig> = HashMap::new();
    for res in resources {
        let key = format!("{}.{}", res.resource_type, res.logical_name);
        lookup.insert(key, res);
    }

    // Regex to detect resource references in string values
    let re = Regex::new(r"([a-z][a-z0-9]*_[a-z][a-z0-9_]*)\.([a-z_][a-z0-9_]*)").unwrap();

    // Build edges by scanning field values
    let mut edges: Vec<GraphEdge> = Vec::new();
    // Track incoming edges per resource id
    let mut incoming: HashMap<String, HashSet<String>> = HashMap::new();
    for res in resources {
        incoming.entry(res.id.clone()).or_default();
    }

    for res in resources {
        for (field_name, value) in &res.fields {
            let text = match value {
                serde_json::Value::String(s) => s.clone(),
                _ => value.to_string(),
            };

            for cap in re.captures_iter(&text) {
                let ref_key = format!("{}.{}", &cap[1], &cap[2]);
                if let Some(target) = lookup.get(&ref_key) {
                    if target.id != res.id {
                        edges.push(GraphEdge {
                            from_id: res.id.clone(),
                            to_id: target.id.clone(),
                            label: field_name.clone(),
                        });
                        incoming
                            .entry(target.id.clone())
                            .or_default()
                            .insert(res.id.clone());
                    }
                }
            }
        }
    }

    // Topological layering using BFS (Kahn's algorithm variant)
    // outgoing edges: from_id depends on to_id, so to_id should be in an earlier layer
    // We reverse: nodes with no incoming edges go in layer 0
    let mut layers: HashMap<String, usize> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    // Find roots (no incoming edges)
    for res in resources {
        if incoming.get(&res.id).map_or(true, |s| s.is_empty()) {
            queue.push_back(res.id.clone());
            layers.insert(res.id.clone(), 0);
        }
    }

    // BFS to assign layers
    // Build adjacency: for each resource, which resources depend on it (have edges pointing to it)
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &edges {
        dependents
            .entry(edge.to_id.clone())
            .or_default()
            .push(edge.from_id.clone());
    }

    while let Some(node_id) = queue.pop_front() {
        let current_layer = *layers.get(&node_id).unwrap_or(&0);
        if let Some(deps) = dependents.get(&node_id) {
            for dep_id in deps {
                let new_layer = current_layer + 1;
                let existing = layers.get(dep_id).copied().unwrap_or(0);
                if new_layer > existing {
                    layers.insert(dep_id.clone(), new_layer);
                }
                queue.push_back(dep_id.clone());
            }
        }
    }

    // Assign any unvisited nodes to layer 0
    for res in resources {
        layers.entry(res.id.clone()).or_insert(0);
    }

    // Group by layer for x positioning
    let mut layer_groups: HashMap<usize, Vec<String>> = HashMap::new();
    for res in resources {
        let layer = *layers.get(&res.id).unwrap_or(&0);
        layer_groups.entry(layer).or_default().push(res.id.clone());
    }

    // Build nodes with x/y coordinates
    let node_width = 220.0_f64;
    let row_height = 160.0_f64;
    let mut nodes: Vec<GraphNode> = Vec::new();
    let res_map: HashMap<&str, &TofuResourceConfig> =
        resources.iter().map(|r| (r.id.as_str(), r)).collect();

    for (layer, ids) in &layer_groups {
        for (col, id) in ids.iter().enumerate() {
            if let Some(res) = res_map.get(id.as_str()) {
                nodes.push(GraphNode {
                    id: res.id.clone(),
                    label: format!("{}.{}", res.resource_type, res.logical_name),
                    resource_type: res.resource_type.clone(),
                    provider_id: res.provider_id.clone(),
                    x: col as f64 * node_width,
                    y: *layer as f64 * row_height,
                });
            }
        }
    }

    DependencyGraph { nodes, edges }
}
