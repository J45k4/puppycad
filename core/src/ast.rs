use crate::types::Span;

#[derive(Debug)]
pub struct File {
	pub decls: Vec<Decl>,
	pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
	Solid,
	Feature,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
	pub kind: DeclKind,
	pub id: String,
	pub op: String,
	pub entries: Vec<Entry>,
	pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
	Let {
		name: String,
		expr: Expr,
		span: Span,
	},
	Field {
		name: String,
		expr: Expr,
		span: Span,
	},
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectField {
	pub name: String,
	pub expr: Expr,
	pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
	Neg,
	Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
	Or,
	And,
	Eq,
	Ne,
	Lt,
	Le,
	Gt,
	Ge,
	Add,
	Sub,
	Mul,
	Div,
	Mod,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
	pub kind: ExprKind,
	pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
	Number(f64),
	String(String),
	Bool(bool),
	Null,
	Vector(Box<[Expr; 3]>),
	Object(Vec<ObjectField>),
	Reference(Vec<String>),
	Ident(String),
	Call {
		name: String,
		args: Vec<Expr>,
	},
	Unary {
		op: UnaryOp,
		expr: Box<Expr>,
	},
	Binary {
		op: BinaryOp,
		left: Box<Expr>,
		right: Box<Expr>,
	},
}
