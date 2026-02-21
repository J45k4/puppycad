use std::collections::{HashMap, HashSet};

use crate::ast::{BinaryOp, DeclKind, Entry, Expr, ExprKind, File, UnaryOp};
use crate::feature_graph::FeatureGraph;
use crate::types::{CompiledNode, ErrorCode, ErrorLevel, LangError, Position, Span, Value};

#[derive(Debug, Default)]
pub struct Evaluator<'a> {
	feature_graph: FeatureGraph<'a>,
	resolved: HashMap<String, HashMap<String, Value>>,
	evaluating: HashSet<String>,
}

impl<'a> Evaluator<'a> {
	pub fn new(file: &'a File) -> Self {
		Self {
			feature_graph: FeatureGraph::new(file),
			resolved: HashMap::new(),
			evaluating: HashSet::new(),
		}
	}

	pub fn build(&mut self) -> Result<Vec<CompiledNode>, LangError> {
		let mut nodes = Vec::new();
		let ids = self.feature_graph.declaration_order().to_vec();
		for id in ids {
			let fields = self.resolve_decl(id)?;
			let decl = self.feature_graph.decl(id).ok_or_else(|| LangError {
				level: ErrorLevel::Error,
				code: ErrorCode::UnknownIdentifier,
				message: format!("unknown declaration '{id}'"),
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
			let mut node_fields = serde_json::Map::new();
			for entry in &decl.entries {
				let Entry::Field { name, .. } = entry else {
					continue;
				};
				let value = fields.get(name).expect("field should have been evaluated");
				node_fields.insert(name.clone(), value.to_json());
			}

			let kind = match decl.kind {
				DeclKind::Solid => "solid",
				DeclKind::Feature => "feature",
			};

			nodes.push(CompiledNode {
				id: decl.id.clone(),
				kind: kind.to_owned(),
				op: decl.op.clone(),
				fields: serde_json::Value::Object(node_fields),
			});
		}
		Ok(nodes)
	}

	pub fn resolve_decl(&mut self, id: &str) -> Result<HashMap<String, Value>, LangError> {
		if let Some(cached) = self.resolved.get(id).cloned() {
			return Ok(cached);
		}
		if !self.feature_graph.has_decl(id) {
			return Err(LangError {
				level: ErrorLevel::Error,
				code: ErrorCode::UnknownIdentifier,
				message: format!("unknown declaration '{id}'"),
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
			});
		}
		if !self.evaluating.insert(id.to_owned()) {
			return Err(LangError {
				level: ErrorLevel::Error,
				code: ErrorCode::DependencyCycle,
				message: format!("cycle detected involving '{id}'"),
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
			});
		}

		let decl = self.feature_graph.decl(id).ok_or_else(|| LangError {
			level: ErrorLevel::Error,
			code: ErrorCode::UnknownIdentifier,
			message: format!("unknown declaration '{id}'"),
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
		let mut scope = HashMap::<String, Value>::new();

		for entry in &decl.entries {
			match entry {
				Entry::Let { name, expr, .. } => {
					let value = self.eval_expr(expr, &scope, id)?;
					scope.insert(name.clone(), value);
				}
				Entry::Field { name, expr, .. } => {
					let value = self.eval_expr(expr, &scope, id)?;
					scope.insert(name.clone(), value);
				}
			}
		}

		let mut fields = HashMap::new();
		for entry in &decl.entries {
			let Entry::Field { name, .. } = entry else {
				continue;
			};
			let Some(value) = scope.get(name) else {
				return Err(LangError {
					level: ErrorLevel::Error,
					code: ErrorCode::MissingField,
					message: format!("missing field '{name}' while serializing"),
					span: decl.span,
					node: Some(id.to_owned()),
					details: Vec::new(),
				});
			};
			fields.insert(name.clone(), value.clone());
		}

		self.resolved.insert(id.to_owned(), fields.clone());
		self.evaluating.remove(id);
		Ok(fields)
	}

	fn eval_expr(&mut self, expr: &Expr, scope: &HashMap<String, Value>, current: &str) -> Result<Value, LangError> {
		match &expr.kind {
			ExprKind::Number(value) => Ok(Value::Number(*value)),
			ExprKind::String(value) => Ok(Value::String(value.clone())),
			ExprKind::Bool(value) => Ok(Value::Bool(*value)),
			ExprKind::Null => Ok(Value::Null),
			ExprKind::Vector(values) => Ok(Value::Vec3([
				self
					.eval_expr(&values[0], scope, current)?
					.as_number()
					.ok_or_else(|| self.type_error(expr.span, "expected number for vector x component"))?,
				self
					.eval_expr(&values[1], scope, current)?
					.as_number()
					.ok_or_else(|| self.type_error(expr.span, "expected number for vector y component"))?,
				self
					.eval_expr(&values[2], scope, current)?
					.as_number()
					.ok_or_else(|| self.type_error(expr.span, "expected number for vector z component"))?,
			])),
			ExprKind::Object(entries) => {
				let mut fields = Vec::new();
				for entry in entries {
					let value = self.eval_expr(&entry.expr, scope, current)?;
					fields.push((entry.name.clone(), value));
				}
				Ok(Value::Object(fields))
			}
			ExprKind::Ident(name) => scope
				.get(name)
				.cloned()
				.or_else(|| self.feature_graph.has_decl(name.as_str()).then(|| Value::NodeRef(name.clone())))
				.ok_or_else(|| self.unknown_identifier(expr.span, current, name)),
			ExprKind::Reference(segments) => self.resolve_reference(segments, scope, expr.span, current),
			ExprKind::Call { name, args } => self.eval_call(name, args, scope, current, expr.span),
			ExprKind::Unary { op, expr } => {
				let value = self.eval_expr(expr, scope, current)?;
				match op {
					UnaryOp::Neg => Ok(value
						.as_number()
						.map(Value::Number)
						.ok_or_else(|| self.type_error(expr.span, "unary '-' expects a number"))?),
					UnaryOp::Not => Ok(value
						.as_bool()
						.map(Value::Bool)
						.ok_or_else(|| self.type_error(expr.span, "unary '!' expects a boolean"))?),
				}
			}
			ExprKind::Binary { op, left, right } => {
				let left_value = self.eval_expr(left, scope, current)?;
				let right_value = self.eval_expr(right, scope, current)?;
				match op {
					BinaryOp::Or => {
						let lhs = left_value
							.as_bool()
							.ok_or_else(|| self.type_error(expr.span, "logical '||' expects booleans"))?;
						let rhs = right_value
							.as_bool()
							.ok_or_else(|| self.type_error(expr.span, "logical '||' expects booleans"))?;
						Ok(Value::Bool(lhs || rhs))
					}
					BinaryOp::And => {
						let lhs = left_value
							.as_bool()
							.ok_or_else(|| self.type_error(expr.span, "logical '&&' expects booleans"))?;
						let rhs = right_value
							.as_bool()
							.ok_or_else(|| self.type_error(expr.span, "logical '&&' expects booleans"))?;
						Ok(Value::Bool(lhs && rhs))
					}
					BinaryOp::Eq => self.compare_eq_ne(expr.span, false, &left_value, &right_value),
					BinaryOp::Ne => self.compare_eq_ne(expr.span, true, &left_value, &right_value),
					BinaryOp::Lt => {
						self.compare_numbers(expr.span, left_value.as_number(), right_value.as_number(), |a, b| a < b, "comparison '<'")
					}
					BinaryOp::Le => {
						self.compare_numbers(expr.span, left_value.as_number(), right_value.as_number(), |a, b| a <= b, "comparison '<='")
					}
					BinaryOp::Gt => {
						self.compare_numbers(expr.span, left_value.as_number(), right_value.as_number(), |a, b| a > b, "comparison '>'")
					}
					BinaryOp::Ge => {
						self.compare_numbers(expr.span, left_value.as_number(), right_value.as_number(), |a, b| a >= b, "comparison '>='")
					}
					BinaryOp::Add => {
						self.arith(expr.span, left_value, right_value, |a, b| a + b, "addition")
					}
					BinaryOp::Sub => {
						self.arith(expr.span, left_value, right_value, |a, b| a - b, "subtraction")
					}
					BinaryOp::Mul => {
						self.arith(expr.span, left_value, right_value, |a, b| a * b, "multiplication")
					}
					BinaryOp::Div => {
						self.arith(expr.span, left_value, right_value, |a, b| a / b, "division")
					}
					BinaryOp::Mod => {
						self.arith(expr.span, left_value, right_value, |a, b| a % b, "remainder")
					}
				}
			}
		}
	}

	fn resolve_reference(
		&mut self,
		segments: &[String],
		scope: &HashMap<String, Value>,
		span: Span,
		current: &str,
	) -> Result<Value, LangError> {
		let first = segments.first().ok_or_else(|| self.syntax(span, current, "empty reference"))?;
		if segments.len() == 1 {
			return scope
				.get(first)
				.cloned()
				.ok_or_else(|| self.unknown_identifier(span, current, first));
		}

		let decl_id = first.as_str();
		self.feature_graph
			.decl(decl_id)
			.ok_or_else(|| self.unknown_identifier(span, current, first))?;

		if segments.len() == 2 {
			let field = &segments[1];
			let target_fields = self.resolve_decl(decl_id)?;
			if let Some(value) = target_fields.get(field) {
				return Ok(value.clone());
			}
		}

		let anchor = segments[1].as_str();
		if matches!(anchor, "top" | "bottom" | "left" | "right" | "front" | "back") && segments.len() == 2 {
			return Ok(Value::TargetRef {
				node: decl_id.to_owned(),
				anchor: anchor.to_owned(),
			});
		}

		Err(LangError {
			level: ErrorLevel::Error,
			code: ErrorCode::UnknownIdentifier,
			message: format!("unknown reference '{}'", segments.join(".")),
			span,
			node: Some(current.to_owned()),
			details: vec![(
				"reference".to_owned(),
				format!("{}.{}", segments[0].as_str(), segments[1..].join(".")),
			)],
		})
	}

	fn eval_call(
		&mut self,
		name: &str,
		args: &[Expr],
		scope: &HashMap<String, Value>,
		current: &str,
		span: Span,
	) -> Result<Value, LangError> {
		let values = args.iter().map(|arg| self.eval_expr(arg, scope, current)).collect::<Result<Vec<_>, _>>()?;
		match name {
			"min" => self.binary_fn(span, name, &values, |a, b| a.min(b)),
			"max" => self.binary_fn(span, name, &values, |a, b| a.max(b)),
			"abs" => self.unary_fn(span, name, values.first().cloned(), |value| value.abs()),
			"sqrt" => self.unary_fn(span, name, values.first().cloned(), |value| value.sqrt()),
			"sin" => self.unary_fn(span, name, values.first().cloned(), |value| value.sin()),
			"cos" => self.unary_fn(span, name, values.first().cloned(), |value| value.cos()),
			"tan" => self.unary_fn(span, name, values.first().cloned(), |value| value.tan()),
			"clamp" => {
				if values.len() != 3 {
					return Err(self.wrong_arity(span, name, 3, values.len()));
				}
				let value = values[0].as_number().ok_or_else(|| self.type_error(span, "clamp expects number arguments"))?;
				let min = values[1].as_number().ok_or_else(|| self.type_error(span, "clamp expects number arguments"))?;
				let max = values[2].as_number().ok_or_else(|| self.type_error(span, "clamp expects number arguments"))?;
				Ok(Value::Number(value.clamp(min, max)))
			}
			"deg" => self.unary_fn(span, name, values.first().cloned(), |value| value.to_radians()),
			"rad" => self.unary_fn(span, name, values.first().cloned(), |value| value),
			"vec3" => {
				if values.len() != 3 {
					return Err(self.wrong_arity(span, name, 3, values.len()));
				}
				Ok(Value::Vec3([
					values[0].as_number().ok_or_else(|| self.type_error(span, "vec3 expects numeric arguments"))?,
					values[1].as_number().ok_or_else(|| self.type_error(span, "vec3 expects numeric arguments"))?,
					values[2].as_number().ok_or_else(|| self.type_error(span, "vec3 expects numeric arguments"))?,
				]))
			}
			_ => Err(LangError {
				level: ErrorLevel::Error,
				code: ErrorCode::UnknownIdentifier,
				message: format!("unknown function '{name}'"),
				span,
				node: Some(current.to_owned()),
				details: Vec::new(),
			}),
		}
	}

	fn compare_eq_ne(&self, span: Span, negate: bool, left: &Value, right: &Value) -> Result<Value, LangError> {
		let value = match (left, right) {
			(Value::Number(lhs), Value::Number(rhs)) => lhs == rhs,
			(Value::Bool(lhs), Value::Bool(rhs)) => lhs == rhs,
			(Value::String(lhs), Value::String(rhs)) => lhs == rhs,
			(Value::Null, Value::Null) => true,
			_ => return Err(self.type_error(span, "cannot compare values with '=='")),
		};
		Ok(Value::Bool(if negate { !value } else { value }))
	}

	fn compare_numbers(
		&self,
		span: Span,
		left: Option<f64>,
		right: Option<f64>,
		op: impl FnOnce(f64, f64) -> bool,
		label: &str,
	) -> Result<Value, LangError> {
		let left = left.ok_or_else(|| self.type_error(span, label))?;
		let right = right.ok_or_else(|| self.type_error(span, label))?;
		Ok(Value::Bool(op(left, right)))
	}

	fn arith(
		&self,
		span: Span,
		left: Value,
		right: Value,
		op: impl FnOnce(f64, f64) -> f64,
		label: &str,
	) -> Result<Value, LangError> {
		let left = left.as_number().ok_or_else(|| self.type_error(span, label))?;
		let right = right.as_number().ok_or_else(|| self.type_error(span, label))?;
		Ok(Value::Number(op(left, right)))
	}

	fn unary_fn(&self, span: Span, name: &str, value: Option<Value>, op: impl FnOnce(f64) -> f64) -> Result<Value, LangError> {
		if value.is_none() {
			return Err(self.wrong_arity(span, name, 1, 0));
		}
		let value = value
			.and_then(|value| value.as_number())
			.ok_or_else(|| self.type_error(span, format!("'{name}' expects a numeric argument")))?;
		Ok(Value::Number(op(value)))
	}

	fn binary_fn(&self, span: Span, name: &str, values: &[Value], op: impl FnOnce(f64, f64) -> f64) -> Result<Value, LangError> {
		if values.len() != 2 {
			return Err(self.wrong_arity(span, name, 2, values.len()));
		}
		let left = values[0]
			.as_number()
			.ok_or_else(|| self.type_error(span, format!("'{name}' expects numeric arguments")))?;
		let right = values[1]
			.as_number()
			.ok_or_else(|| self.type_error(span, format!("'{name}' expects numeric arguments")))?;
		Ok(Value::Number(op(left, right)))
	}

	fn syntax(&self, span: Span, node: &str, message: impl Into<String>) -> LangError {
		LangError {
			level: ErrorLevel::Error,
			code: ErrorCode::SyntaxError,
			message: message.into(),
			span,
			node: Some(node.to_owned()),
			details: Vec::new(),
		}
	}

	fn type_error(&self, span: Span, message: impl Into<String>) -> LangError {
		LangError {
			level: ErrorLevel::Error,
			code: ErrorCode::TypeMismatch,
			message: message.into(),
			span,
			node: None,
			details: Vec::new(),
		}
	}

	fn unknown_identifier(&self, span: Span, node: &str, ident: &str) -> LangError {
		LangError {
			level: ErrorLevel::Error,
			code: ErrorCode::UnknownIdentifier,
			message: format!("unknown identifier '{ident}'"),
			span,
			node: Some(node.to_owned()),
			details: Vec::new(),
		}
	}

	fn wrong_arity(&self, span: Span, name: &str, expected: usize, got: usize) -> LangError {
		LangError {
			level: ErrorLevel::Error,
			code: ErrorCode::TypeMismatch,
			message: format!("function '{name}' expects {expected} args, got {got}"),
			span,
			node: None,
			details: Vec::new(),
		}
	}
}
