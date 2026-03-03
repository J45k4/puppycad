use std::fmt::Write as _;

use crate::ast::{BinaryOp, Decl, DeclKind, Entry, Expr, ExprKind, File, ObjectField, UnaryOp};

const INDENT: &str = "  ";

pub fn format_file(file: &File) -> String {
    let mut output = String::new();
    for (index, decl) in file.decls.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        write_decl(&mut output, decl);
    }
    output
}

fn write_decl(output: &mut String, decl: &Decl) {
    let kind = match decl.kind {
        DeclKind::Solid => "solid",
        DeclKind::Feature => "feature",
    };
    let _ = writeln!(output, "{kind} {} = {} {{", decl.id, decl.op);

    let mut lets = Vec::new();
    let mut fields = Vec::new();
    for entry in &decl.entries {
        match entry {
            Entry::Let { .. } => lets.push(entry),
            Entry::Field { .. } => fields.push(entry),
        }
    }

    fields.sort_by(|left, right| {
        let left_name = match left {
            Entry::Field { name, .. } => name.as_str(),
            Entry::Let { .. } => "",
        };
        let right_name = match right {
            Entry::Field { name, .. } => name.as_str(),
            Entry::Let { .. } => "",
        };
        left_name.cmp(right_name)
    });

    for entry in lets.into_iter().chain(fields.into_iter()) {
        match entry {
            Entry::Let { name, expr, .. } => {
                let _ = writeln!(output, "{INDENT}let {name} = {};", format_expr(expr, 0, 1));
            }
            Entry::Field { name, expr, .. } => {
                let _ = writeln!(output, "{INDENT}{name}: {};", format_expr(expr, 0, 1));
            }
        }
    }

    output.push_str("}\n");
}

fn format_expr(expr: &Expr, parent_precedence: u8, indent_level: usize) -> String {
    match &expr.kind {
        ExprKind::Number(value) => format_number(*value),
        ExprKind::String(value) => format!("\"{}\"", escape_string(value)),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Null => "null".to_owned(),
        ExprKind::Vector(values) => {
            let values = values
                .iter()
                .map(|value| format_expr(value, 0, indent_level))
                .collect::<Vec<_>>();
            format!("[{}, {}, {}]", values[0], values[1], values[2])
        }
        ExprKind::Object(fields) => format_object_expr(fields, indent_level),
        ExprKind::Reference(segments) => segments.join("."),
        ExprKind::Ident(name) => name.clone(),
        ExprKind::Call { name, args } => {
            let args = args
                .iter()
                .map(|arg| format_expr(arg, 0, indent_level))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}({args})")
        }
        ExprKind::Unary { op, expr } => {
            let op_text = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            let precedence = precedence_for_unary();
            let inner = format_expr(expr, precedence, indent_level);
            let rendered = format!("{op_text}{inner}");
            if precedence < parent_precedence {
                format!("({rendered})")
            } else {
                rendered
            }
        }
        ExprKind::Binary { op, left, right } => {
            let precedence = precedence_for_binary(*op);
            let left = format_expr(left, precedence, indent_level);
            let right = format_expr(right, precedence + 1, indent_level);
            let op_text = binary_op_text(*op);
            let rendered = format!("{left} {op_text} {right}");
            if precedence < parent_precedence {
                format!("({rendered})")
            } else {
                rendered
            }
        }
    }
}

fn format_object_expr(fields: &[ObjectField], indent_level: usize) -> String {
    if fields.is_empty() {
        return "{}".to_owned();
    }

    let mut output = String::new();
    output.push_str("{\n");
    for field in fields {
        output.push_str(&INDENT.repeat(indent_level + 1));
        let _ = writeln!(
            output,
            "{}: {};",
            field.name,
            format_expr(&field.expr, 0, indent_level + 1)
        );
    }
    output.push_str(&INDENT.repeat(indent_level));
    output.push('}');
    output
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        let mut text = value.to_string();
        if text.contains('.') {
            while text.ends_with('0') {
                text.pop();
            }
            if text.ends_with('.') {
                text.push('0');
            }
        }
        text
    }
}

fn escape_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn precedence_for_unary() -> u8 {
    7
}

fn precedence_for_binary(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Or => 1,
        BinaryOp::And => 2,
        BinaryOp::Eq | BinaryOp::Ne => 3,
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => 4,
        BinaryOp::Add | BinaryOp::Sub => 5,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 6,
    }
}

fn binary_op_text(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Or => "||",
        BinaryOp::And => "&&",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_pcad;

    use super::*;

    #[test]
    fn formats_lets_before_fields_and_sorts_fields() {
        let source = r#"
feature hole1 = hole {
  y: body.h / 2;
  let cx = body.w / 2;
  x: cx;
  target: body.top;
  d: 5;
}
"#;
        let file = parse_pcad(source).expect("source should parse");
        let formatted = format_file(&file);
        let expected = r#"feature hole1 = hole {
  let cx = body.w / 2;
  d: 5;
  target: body.top;
  x: cx;
  y: body.h / 2;
}
"#;
        assert_eq!(formatted, expected);
    }

    #[test]
    fn formats_object_literals_with_multiline_structure() {
        let source = r#"
feature ch1 = chamfer {
  target: { kind: "face"; of: body; normal: [0, 0, 1]; };
  dist: 2;
}
"#;
        let file = parse_pcad(source).expect("source should parse");
        let formatted = format_file(&file);
        let expected = r#"feature ch1 = chamfer {
  dist: 2;
  target: {
    kind: "face";
    of: body;
    normal: [0, 0, 1];
  };
}
"#;
        assert_eq!(formatted, expected);
    }

    #[test]
    fn format_output_round_trips_through_parser() {
        let source = r#"
solid body = box {
  w: 100;
  h: 50;
  d: 30;
}

feature hole1 = hole {
  let cx = body.w / 2;
  let cy = body.h / 2;
  target: body.top;
  x: cx + 1 * (2 + 3);
  y: cy;
  d: 5;
}
"#;
        let file = parse_pcad(source).expect("source should parse");
        let formatted = format_file(&file);
        let reparsed = parse_pcad(&formatted).expect("formatted source should parse");
        let reformatted = format_file(&reparsed);
        assert_eq!(formatted, reformatted);
    }
}
