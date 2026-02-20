use std::fmt;

use serde_json::{Map, Value as JsonValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub col: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn merge(a: Span, b: Span) -> Span {
        Span {
            start: a.start,
            end: b.end,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    SyntaxError,
    UnknownIdentifier,
    TypeMismatch,
    DuplicateId,
    DependencyCycle,
    TargetNotFound,
    AmbiguousTarget,
    UnknownField,
    MissingField,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::SyntaxError => "syntax_error",
            ErrorCode::UnknownIdentifier => "unknown_identifier",
            ErrorCode::TypeMismatch => "type_mismatch",
            ErrorCode::DuplicateId => "duplicate_id",
            ErrorCode::DependencyCycle => "dependency_cycle",
            ErrorCode::TargetNotFound => "target_not_found",
            ErrorCode::AmbiguousTarget => "ambiguous_target",
            ErrorCode::UnknownField => "unknown_field",
            ErrorCode::MissingField => "missing_field",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorLevel {
    Error,
    Warning,
}

impl ErrorLevel {
    fn as_str(&self) -> &'static str {
        match self {
            ErrorLevel::Error => "error",
            ErrorLevel::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangError {
    pub level: ErrorLevel,
    pub code: ErrorCode,
    pub message: String,
    pub span: Span,
    pub node: Option<String>,
    pub details: Vec<(String, String)>,
}

impl LangError {
    pub fn syntax(span: Span, message: impl Into<String>) -> Self {
        Self {
            level: ErrorLevel::Error,
            code: ErrorCode::SyntaxError,
            message: message.into(),
            span,
            node: None,
            details: Vec::new(),
        }
    }
}

impl fmt::Display for LangError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let payload = serde_json::json!({
            "level": self.level.as_str(),
            "code": self.code.as_str(),
            "message": self.message,
            "span": {
                "start": {
                    "line": self.span.start.line,
                    "col": self.span.start.col,
                },
                "end": {
                    "line": self.span.end.line,
                    "col": self.span.end.col,
                },
            },
        });
        write!(f, "{}", payload.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    String(String),
    NodeRef(String),
    Null,
    Vec3([f64; 3]),
    Object(Vec<(String, Value)>),
    TargetRef { node: String, anchor: String },
}

impl Value {
    pub fn to_json(&self) -> JsonValue {
        match self {
            Self::Number(value) => JsonValue::from(*value),
            Self::Bool(value) => JsonValue::from(*value),
            Self::String(value) => JsonValue::from(value.clone()),
            Self::NodeRef(value) => {
                let mut object = Map::new();
                object.insert("kind".to_owned(), JsonValue::from("node"));
                object.insert("id".to_owned(), JsonValue::from(value.clone()));
                JsonValue::Object(object)
            }
            Self::Null => JsonValue::Null,
            Self::Vec3(value) => JsonValue::Array(vec![JsonValue::from(value[0]), JsonValue::from(value[1]), JsonValue::from(value[2])]),
            Self::Object(fields) => {
                let mut object = Map::new();
                for (name, value) in fields {
                    object.insert(name.clone(), value.to_json());
                }
                JsonValue::Object(object)
            }
            Self::TargetRef { node, anchor } => {
                let mut object = Map::new();
                object.insert("kind".to_owned(), JsonValue::from("target"));
                object.insert("of".to_owned(), JsonValue::from(node.clone()));
                object.insert("anchor".to_owned(), JsonValue::from(anchor.clone()));
                JsonValue::Object(object)
            }
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct CompiledNode {
    pub id: String,
    pub kind: String,
    pub op: String,
    pub fields: JsonValue,
}
