use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::{DeclKind, File};
use crate::feature_graph::FeatureGraph;

const LAYOUT_X_SPACING: f32 = 5.0;
const LAYOUT_Y_SPACING: f32 = 2.5;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeGraph {
    pub nodes: Vec<NodeGraphNode>,
    pub edges: Vec<NodeGraphEdge>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeGraphNode {
    pub id: String,
    pub kind: DeclKind,
    pub op: String,
    pub dependencies: Vec<String>,
    pub position: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeGraphEdge {
    pub from: String,
    pub to: String,
}

pub fn build_node_graph(file: &File) -> NodeGraph {
    let graph = FeatureGraph::new(file);
    let declaration_order = graph.declaration_order();

    let mut dependencies_by_id: HashMap<&str, Vec<&str>> = HashMap::new();
    for id in declaration_order {
        let dependencies = graph
            .dependencies(id)
            .map(|items| items.iter().copied().collect::<Vec<&str>>())
            .unwrap_or_default();
        dependencies_by_id.insert(id, dependencies);
    }

    let topo_order = topological_order(declaration_order, &dependencies_by_id);
    let depth_by_id = compute_depths(declaration_order, &topo_order, &dependencies_by_id);

    let mut level_ids: HashMap<usize, Vec<&str>> = HashMap::new();
    for id in declaration_order {
        let depth = *depth_by_id.get(id).unwrap_or(&0);
        level_ids.entry(depth).or_default().push(id);
    }

    let mut position_by_id = HashMap::new();
    for (depth, ids) in level_ids {
        let center = (ids.len().saturating_sub(1)) as f32 / 2.0;
        for (index, id) in ids.into_iter().enumerate() {
            let x = depth as f32 * LAYOUT_X_SPACING;
            let y = (index as f32 - center) * LAYOUT_Y_SPACING;
            position_by_id.insert(id, [x, y]);
        }
    }

    let mut nodes = Vec::new();
    for id in declaration_order {
        let decl = graph
            .decl(id)
            .expect("declaration id from declaration_order must exist");
        let dependencies = dependencies_by_id
            .get(id)
            .into_iter()
            .flatten()
            .map(|value| (*value).to_owned())
            .collect::<Vec<String>>();
        let position = position_by_id.get(id).copied().unwrap_or([0.0, 0.0]);
        nodes.push(NodeGraphNode {
            id: decl.id.clone(),
            kind: decl.kind,
            op: decl.op.clone(),
            dependencies,
            position,
        });
    }

    let mut edges = Vec::new();
    for node in &nodes {
        for dependency in &node.dependencies {
            edges.push(NodeGraphEdge {
                from: dependency.clone(),
                to: node.id.clone(),
            });
        }
    }

    NodeGraph { nodes, edges }
}

fn topological_order<'a>(
    declaration_order: &[&'a str],
    dependencies_by_id: &HashMap<&'a str, Vec<&'a str>>,
) -> Vec<&'a str> {
    let mut in_degree: HashMap<&'a str, usize> = HashMap::new();
    let mut dependents: HashMap<&'a str, Vec<&'a str>> = HashMap::new();

    for &id in declaration_order {
        in_degree.insert(id, 0);
    }

    for &id in declaration_order {
        let dependencies = dependencies_by_id.get(id).cloned().unwrap_or_default();
        for dependency in dependencies {
            if !in_degree.contains_key(dependency) {
                continue;
            }
            *in_degree
                .get_mut(id)
                .expect("node id must exist in in_degree") += 1;
            dependents.entry(dependency).or_default().push(id);
        }
    }

    let mut queue = VecDeque::new();
    for &id in declaration_order {
        if in_degree.get(id).copied().unwrap_or(0) == 0 {
            queue.push_back(id);
        }
    }

    let mut ordered = Vec::with_capacity(declaration_order.len());
    while let Some(id) = queue.pop_front() {
        ordered.push(id);
        if let Some(children) = dependents.get(id) {
            for &child in children {
                let degree = in_degree
                    .get_mut(child)
                    .expect("dependent node must exist in in_degree");
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    queue.push_back(child);
                }
            }
        }
    }

    for &id in declaration_order {
        if !ordered.contains(&id) {
            ordered.push(id);
        }
    }

    ordered
}

fn compute_depths<'a>(
    declaration_order: &[&'a str],
    topo_order: &[&'a str],
    dependencies_by_id: &HashMap<&'a str, Vec<&'a str>>,
) -> HashMap<&'a str, usize> {
    let mut depths: HashMap<&str, usize> = HashMap::new();
    let declaration_set: HashSet<&str> = declaration_order.iter().copied().collect();

    for &id in topo_order {
        let mut depth = 0usize;
        for dependency in dependencies_by_id.get(id).into_iter().flatten() {
            if !declaration_set.contains(dependency) {
                continue;
            }
            let candidate = depths
                .get(dependency)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            depth = depth.max(candidate);
        }
        depths.insert(id, depth);
    }

    for &id in declaration_order {
        depths.entry(id).or_insert(0);
    }

    depths
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_pcad;

    use super::*;

    #[test]
    fn builds_dependency_edges_and_layout_for_linear_chain() {
        let source = r#"
solid body = box {
  w: 20;
  h: 20;
  d: 20;
}

feature hole = hole {
  target: body.top;
  x: 10;
  y: 10;
  d: 4;
}
"#;
        let file = parse_pcad(source).expect("expected source to parse");
        let graph = build_node_graph(&file);

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(
            graph.edges[0],
            NodeGraphEdge {
                from: "body".to_owned(),
                to: "hole".to_owned()
            }
        );

        let body = graph
            .nodes
            .iter()
            .find(|node| node.id == "body")
            .expect("body node should exist");
        let hole = graph
            .nodes
            .iter()
            .find(|node| node.id == "hole")
            .expect("hole node should exist");
        assert!(hole.position[0] > body.position[0]);
    }

    #[test]
    fn independent_nodes_share_same_depth_column() {
        let source = r#"
solid a = box {
  w: 1;
  h: 2;
  d: 3;
}

solid b = box {
  w: 2;
  h: 3;
  d: 4;
}
"#;
        let file = parse_pcad(source).expect("expected source to parse");
        let graph = build_node_graph(&file);
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 0);
        assert_eq!(graph.nodes[0].position[0], graph.nodes[1].position[0]);
    }

    #[test]
    fn retains_nodes_even_when_dependency_cycle_exists() {
        let source = r#"
solid a = box {
  w: b.w;
  h: 1;
  d: 1;
}

solid b = box {
  w: a.w;
  h: 1;
  d: 1;
}
"#;
        let file = parse_pcad(source).expect("expected source to parse");
        let graph = build_node_graph(&file);
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 2);
    }
}
