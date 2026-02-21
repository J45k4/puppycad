use std::collections::{HashMap, HashSet};

use crate::ast::{Decl, Entry, Expr, ExprKind, File};

#[derive(Debug, Default)]
pub struct FeatureGraph<'a> {
	decls: HashMap<&'a str, &'a Decl>,
	order: Vec<&'a str>,
	dependencies: HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> FeatureGraph<'a> {
	pub fn new(file: &'a File) -> Self {
		let mut decls = HashMap::new();
		let mut order = Vec::new();
		for decl in &file.decls {
			decls.insert(decl.id.as_str(), decl);
			order.push(decl.id.as_str());
		}

		let mut dependencies = HashMap::new();
		for decl in &file.decls {
			let mut deps = Vec::new();
			let mut local_names = HashSet::new();
			for entry in &decl.entries {
				match entry {
					Entry::Let { expr, .. } => visit_expr(expr, &decls, &local_names, &mut deps),
					Entry::Field { expr, .. } => visit_expr(expr, &decls, &local_names, &mut deps),
				}

				match entry {
					Entry::Let { name, .. } => {
						local_names.insert(name.as_str());
					}
					Entry::Field { name, .. } => {
						local_names.insert(name.as_str());
					}
				}
			}
			deps.sort_unstable();
			deps.dedup();
			dependencies.insert(decl.id.as_str(), deps);
		}

		Self {
			decls,
			order,
			dependencies,
		}
	}

	pub fn declaration_order(&self) -> &[&'a str] {
		&self.order
	}

	pub fn decl(&self, id: &str) -> Option<&'a Decl> {
		self.decls.get(id).copied()
	}

	pub fn dependencies(&self, id: &str) -> Option<&Vec<&'a str>> {
		self.dependencies.get(id)
	}

	pub fn has_decl(&self, id: &str) -> bool {
		self.decls.contains_key(id)
	}

	pub fn decls(&self) -> &HashMap<&'a str, &'a Decl> {
		&self.decls
	}
}

fn visit_expr<'a>(
	expr: &Expr,
	decls: &HashMap<&'a str, &'a Decl>,
	local_names: &HashSet<&'a str>,
	out: &mut Vec<&'a str>,
) {
	match &expr.kind {
		ExprKind::Vector(values) => {
			for value in values.iter() {
				visit_expr(value, decls, local_names, out);
			}
		}
		ExprKind::Object(fields) => {
			for field in fields {
				visit_expr(&field.expr, decls, local_names, out);
			}
		}
		ExprKind::Ident(ident) => {
			if local_names.contains(ident.as_str()) {
				return;
			}
			if let Some(other_decl) = decls.get(ident.as_str()) {
				out.push(other_decl.id.as_str());
			}
		}
		ExprKind::Reference(segments) => {
			let Some(first) = segments.first() else {
				return;
			};
			if local_names.contains(first.as_str()) {
				return;
			}
			if let Some(target) = decls.get(first.as_str()) {
				out.push(target.id.as_str());
			}
		}
		ExprKind::Unary { expr, .. } => visit_expr(expr, decls, local_names, out),
		ExprKind::Binary { left, right, .. } => {
			visit_expr(left, decls, local_names, out);
			visit_expr(right, decls, local_names, out);
		}
		ExprKind::Call { args, .. } => {
			for arg in args {
				visit_expr(arg, decls, local_names, out);
			}
		}
		ExprKind::Number(_) | ExprKind::String(_) | ExprKind::Bool(_) | ExprKind::Null => {}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::parser::parse_pcad;

	#[test]
	fn feature_graph_collects_dependencies_in_declaration_order() {
		let src = r#"
			solid body = box {
				w: 20;
				h: 20;
				d: 20;
			}

			feature hole = hole {
				target: body.top;
				let cx = body.w;
				let cy = body.h;
			}
		"#;
		let file = parse_pcad(src).expect("should parse");
		let graph = FeatureGraph::new(&file);

		assert_eq!(graph.declaration_order(), &["body", "hole"]);
		assert!(graph.dependencies("body").expect("body exists").is_empty());
		assert_eq!(graph.dependencies("hole").expect("hole exists"), &["body"]);
	}

	#[test]
	fn feature_graph_treats_later_locals_as_unknown_until_defined() {
		let src = r#"
			solid body = box {
				w: 20;
				h: 20;
				d: 20;
			}

			feature f = hole {
				target: body;
				let body = 0;
			}
		"#;
		let file = parse_pcad(src).expect("should parse");
		let graph = FeatureGraph::new(&file);

		assert_eq!(graph.dependencies("f").expect("feature exists"), &["body"]);
	}

	#[test]
	fn feature_graph_deduplicates_dependencies() {
		let src = r#"
			solid body = box {
				w: 20;
				h: 20;
				d: 20;
			}

			feature f = hole {
				x: body.w + body.w;
				y: body.h + body.h;
			}
		"#;
		let file = parse_pcad(src).expect("should parse");
		let graph = FeatureGraph::new(&file);

		assert_eq!(graph.dependencies("f").expect("feature exists"), &["body"]);
	}
}
