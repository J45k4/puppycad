use serde_json::{Map, Value as JsonValue};

use crate::parser::File;
use crate::eval::Evaluator;
use crate::types::{CompiledNode, ErrorCode, ErrorLevel, LangError};

pub fn compile_to_three_json(file: &File) -> Result<String, LangError> {
    let mut evaluator = Evaluator::new(file);
    let mut nodes: Vec<CompiledNode> = evaluator.build()?;
    let final_id = file.decls.last().map_or(String::new(), |decl| decl.id.clone());

    let mut body = Map::new();
    body.insert(
        "version".to_owned(),
        JsonValue::from("puppycad.featuregraph@0.1"),
    );
    let node_values: Vec<JsonValue> = nodes
        .drain(..)
        .map(|node| {
            let mut data = Map::new();
            data.insert("id".to_owned(), JsonValue::from(node.id));
            data.insert("kind".to_owned(), JsonValue::from(node.kind));
            data.insert("op".to_owned(), JsonValue::from(node.op));
            data.insert("fields".to_owned(), node.fields);
            JsonValue::Object(data)
        })
        .collect();
    body.insert("nodes".to_owned(), JsonValue::Array(node_values));
    body.insert("finalId".to_owned(), JsonValue::from(final_id));

    serde_json::to_string_pretty(&JsonValue::Object(body)).map_err(|err| LangError {
        level: ErrorLevel::Error,
        code: ErrorCode::SyntaxError,
        message: format!("failed to emit JSON: {err}"),
        span: file.span,
        node: None,
        details: Vec::new(),
    })
}
