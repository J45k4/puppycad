use std::collections::{HashMap, HashSet};

use crate::ast::{DeclKind, File};
use crate::eval::Evaluator;
use crate::feature_graph::FeatureGraph;
use crate::types::{ErrorCode, ErrorLevel, LangError, ModelNode, ModelState, Position, Span};

pub fn build_model_state<'a>(graph: &FeatureGraph<'a>) -> Result<ModelState, LangError> {
	let execution_order = topological_order(graph)?;
	let declaration_order: Vec<String> = graph.declaration_order().iter().map(|id| (*id).to_owned()).collect();
	let file = file_from_graph(graph)?;

	let mut evaluator = Evaluator::new(&file);
	let mut nodes = HashMap::new();
	for id in &execution_order {
		let fields = evaluator.resolve_decl(id)?;
		let decl = graph.decl(id).ok_or_else(|| LangError {
			level: ErrorLevel::Error,
			code: ErrorCode::UnknownIdentifier,
			message: format!("unknown declaration '{id}' while building model state"),
			span: Span {
				start: Position {
					line: 0,
					col: 0,
					offset: 0,
				},
				end: Position {
					line: 0,
					col: 0,
					offset: 0,
				},
			},
			node: Some((*id).to_owned()),
			details: Vec::new(),
		})?;
		let kind = match decl.kind {
			DeclKind::Solid => "solid".to_owned(),
			DeclKind::Feature => "feature".to_owned(),
		};

		nodes.insert(
			id.to_owned().to_string(),
			ModelNode {
				id: decl.id.clone(),
				kind,
				op: decl.op.clone(),
				fields,
				dependencies: graph
					.dependencies(id)
					.map(|dependencies| dependencies.iter().map(|dep| (*dep).to_owned()).collect())
					.unwrap_or_default(),
				span: decl.span,
			},
		);
	}

	Ok(ModelState {
		nodes,
		declaration_order,
		execution_order: execution_order.iter().map(|id| (*id).to_owned()).collect(),
		final_node_id: file.decls.last().map(|decl| decl.id.clone()),
	})
}

fn file_from_graph<'a>(graph: &FeatureGraph<'a>) -> Result<File, LangError> {
	let mut decls = Vec::new();
	for id in graph.declaration_order() {
		let decl = graph.decl(id).ok_or_else(|| LangError {
			level: ErrorLevel::Error,
			code: ErrorCode::UnknownIdentifier,
			message: format!("missing declaration '{id}' while building synthetic file"),
			span: Span {
				start: Position {
					line: 0,
					col: 0,
					offset: 0,
				},
				end: Position {
					line: 0,
					col: 0,
					offset: 0,
				},
			},
			node: Some((*id).to_owned()),
			details: Vec::new(),
		})?;
		decls.push(decl.clone());
	}

	let span = decls
		.first()
		.map(|decl| decl.span)
		.unwrap_or(Span {
			start: Position {
				line: 0,
				col: 0,
				offset: 0,
			},
			end: Position {
				line: 0,
				col: 0,
				offset: 0,
			},
		});
	Ok(File { decls, span })
}

fn topological_order<'a>(graph: &FeatureGraph<'a>) -> Result<Vec<&'a str>, LangError> {
	let order = graph.declaration_order();
	let mut in_degree: HashMap<&'a str, usize> = HashMap::new();
	let mut dependents: HashMap<&'a str, Vec<&'a str>> = HashMap::new();

	for &id in order.iter() {
		in_degree.insert(id, 0);
	}

	for &id in order.iter() {
		let Some(deps) = graph.dependencies(id) else {
			continue;
		};
		for &dep in deps.iter() {
			if !graph.has_decl(dep) {
				continue;
			}
			let entry = in_degree.get_mut(id).ok_or_else(|| LangError {
				level: ErrorLevel::Error,
				code: ErrorCode::UnknownIdentifier,
				message: format!("unknown declaration '{id}' while computing dependency graph"),
				span: Span {
					start: Position {
						line: 0,
						col: 0,
						offset: 0,
					},
					end: Position {
						line: 0,
						col: 0,
						offset: 0,
					},
				},
				node: Some(id.to_owned()),
				details: Vec::new(),
			})?;
			*entry += 1;
			dependents.entry(dep).or_default().push(id);
		}
	}

	let mut visited = HashSet::new();
	let mut ordered = Vec::with_capacity(order.len());
	for _ in 0..order.len() {
		let mut next = None;
		for &id in order.iter() {
			if visited.contains(id) {
				continue;
			}
			let degree = *in_degree.get(id).unwrap_or(&0);
			if degree == 0 {
				next = Some(id);
				break;
			}
		}
		let Some(id) = next else {
			let unresolved = order
				.iter()
				.copied()
				.filter(|id| !visited.contains(id))
				.collect::<Vec<&'a str>>();
			let cycle_span = unresolved
				.first()
				.and_then(|id| graph.decl(*id))
				.map(|decl| decl.span)
				.unwrap_or(Span {
					start: Position {
						line: 0,
						col: 0,
						offset: 0,
					},
					end: Position {
						line: 0,
						col: 0,
						offset: 0,
					},
				});
			return Err(LangError {
				level: ErrorLevel::Error,
				code: ErrorCode::DependencyCycle,
				message: format!("dependency cycle detected: {}", unresolved.join(", ")),
				span: cycle_span,
				node: unresolved.first().map(|id| (*id).to_owned()),
				details: Vec::new(),
			});
		};

		visited.insert(id);
		ordered.push(id);

		if let Some(children) = dependents.get(id) {
			for &child in children {
				let Some(child_degree) = in_degree.get_mut(child) else {
					continue;
				};
				if *child_degree > 0 {
					*child_degree -= 1;
				}
			}
		}
	}

	Ok(ordered)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::parser::parse_pcad;

	#[test]
	fn build_model_state_uses_topological_order() {
		let source = r#"
solid body = box {
  w: 20;
  h: 20;
  d: 20;
}

feature hole = hole {
  target: body.top;
  x: 0;
  y: 0;
  d: 5;
}
"#;
		let file = parse_pcad(source).expect("should parse");
		let graph = crate::feature_graph::FeatureGraph::new(&file);
		let state = build_model_state(&graph).expect("should build model state");

		assert_eq!(state.execution_order, vec!["body", "hole"]);
		assert_eq!(state.final_node_id.as_deref(), Some("hole"));
		assert_eq!(state.nodes.len(), 2);
		assert_eq!(
			state.nodes.get("hole").expect("hole exists").dependencies,
			vec!["body".to_owned()]
		);
	}

	#[test]
	fn build_model_state_returns_cycle_error() {
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
		let file = parse_pcad(source).expect("should parse");
		let graph = crate::feature_graph::FeatureGraph::new(&file);
		let err = build_model_state(&graph).expect_err("should fail for cycle");

		assert_eq!(err.code, ErrorCode::DependencyCycle);
	}
}
