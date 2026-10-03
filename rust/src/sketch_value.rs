//! Dimension expressions and sketch variables, ported from
//! `src/sketch-value.ts` and `src/sketch-variables.ts`.
//!
//! Values are stored in millimeters and degrees. Expressions support
//! `+ - * / ( )`, `pi`, numbers with optional units (`mm cm m um in inch ft
//! deg rad`) and `#name` references to sketch variables. They are evaluated,
//! never executed.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Scalar,
    Length,
    Angle,
}

impl ValueKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::Length => "length",
            Self::Angle => "angle",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "scalar" => Some(Self::Scalar),
            "length" => Some(Self::Length),
            "angle" => Some(Self::Angle),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    pub value: f64,
    pub kind: ValueKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SketchVariable {
    pub name: String,
    pub kind: ValueKind,
    pub expression: String,
}

fn unit(token: &str) -> Option<Quantity> {
    let (value, kind) = match token {
        "mm" => (1.0, ValueKind::Length),
        "cm" => (10.0, ValueKind::Length),
        "m" => (1000.0, ValueKind::Length),
        "um" | "µm" | "μm" => (0.001, ValueKind::Length),
        "in" | "inch" => (25.4, ValueKind::Length),
        "ft" => (304.8, ValueKind::Length),
        "deg" | "°" => (1.0, ValueKind::Angle),
        "rad" => (180.0 / std::f64::consts::PI, ValueKind::Angle),
        _ => return None,
    };
    Some(Quantity { value, kind })
}

fn is_number(token: &str) -> bool {
    !token.is_empty() && token.parse::<f64>().is_ok() && !token.starts_with(['+', '-'])
}

fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let letter = |c: char| c.is_ascii_alphabetic() || matches!(c, 'µ' | 'μ' | '°');
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '#'
            && chars
                .get(i + 1)
                .is_some_and(|n| n.is_ascii_alphabetic() || *n == '_')
        {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < chars.len() && matches!(chars[i], 'e' | 'E') {
                let mut j = i + 1;
                if j < chars.len() && matches!(chars[j], '+' | '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    i = j;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            tokens.push(chars[start..i].iter().collect());
        } else if letter(c) {
            let start = i;
            while i < chars.len() && letter(chars[i]) {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect::<String>().to_lowercase());
        } else {
            tokens.push(c.to_string());
            i += 1;
        }
    }
    tokens
}

struct Parser<'a> {
    tokens: Vec<String>,
    index: usize,
    resolve: &'a mut dyn FnMut(&str) -> Result<Quantity, String>,
}

fn finite(q: Quantity) -> Result<Quantity, String> {
    if q.value.is_finite() {
        Ok(q)
    } else {
        Err("Dimension must be finite; division by zero is not allowed.".into())
    }
}

impl Parser<'_> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.index).map(String::as_str)
    }

    fn next(&mut self) -> Option<String> {
        let token = self.tokens.get(self.index).cloned();
        self.index += 1;
        token
    }

    fn primary(&mut self, depth: usize) -> Result<Quantity, String> {
        if depth > 32 {
            return Err("Dimension expression is nested too deeply.".into());
        }
        let token = self.next();
        if let Some(sign @ ("+" | "-")) = token.as_deref() {
            let q = self.primary(depth + 1)?;
            return Ok(Quantity {
                value: if sign == "-" { -q.value } else { q.value },
                ..q
            });
        }
        let mut q = match token.as_deref() {
            Some("(") => {
                let q = self.sum(depth + 1)?;
                if self.next().as_deref() != Some(")") {
                    return Err("Missing closing parenthesis.".into());
                }
                q
            }
            Some("pi") => Quantity {
                value: std::f64::consts::PI,
                kind: ValueKind::Scalar,
            },
            Some(name) if name.starts_with('#') => finite((self.resolve)(&name[1..])?)?,
            Some(number) if is_number(number) => finite(Quantity {
                value: number.parse().unwrap_or(f64::NAN),
                kind: ValueKind::Scalar,
            })?,
            _ => return Err("Enter a number, pi, or a parenthesized expression.".into()),
        };
        if let Some(unit) = self.peek().and_then(unit) {
            self.index += 1;
            if q.kind != ValueKind::Scalar {
                return Err("A unit can only follow a dimensionless value.".into());
            }
            q = finite(Quantity {
                kind: unit.kind,
                value: q.value * unit.value,
            })?;
        }
        Ok(q)
    }

    fn product(&mut self, depth: usize) -> Result<Quantity, String> {
        let mut a = self.primary(depth)?;
        while let Some(op @ ("*" | "/")) = self.peek().map(str::to_string).as_deref() {
            self.index += 1;
            let b = self.primary(depth)?;
            a = if op == "*" {
                if a.kind != ValueKind::Scalar && b.kind != ValueKind::Scalar {
                    return Err("Multiplication requires at least one dimensionless value.".into());
                }
                finite(Quantity {
                    kind: if a.kind == ValueKind::Scalar {
                        b.kind
                    } else {
                        a.kind
                    },
                    value: a.value * b.value,
                })?
            } else {
                if b.kind != ValueKind::Scalar && b.kind != a.kind {
                    return Err("Cannot divide incompatible dimensions.".into());
                }
                finite(Quantity {
                    kind: if b.kind == ValueKind::Scalar {
                        a.kind
                    } else {
                        ValueKind::Scalar
                    },
                    value: a.value / b.value,
                })?
            };
        }
        Ok(a)
    }

    fn sum(&mut self, depth: usize) -> Result<Quantity, String> {
        let mut a = self.product(depth)?;
        while let Some(op @ ("+" | "-")) = self.peek().map(str::to_string).as_deref() {
            self.index += 1;
            let b = self.product(depth)?;
            if a.kind != b.kind {
                return Err("Addition and subtraction require matching units; specify units on both values.".into());
            }
            a = finite(Quantity {
                kind: a.kind,
                value: if op == "+" {
                    a.value + b.value
                } else {
                    a.value - b.value
                },
            })?;
        }
        Ok(a)
    }
}

/// Evaluates `text` to a quantity without checking the expected kind.
pub fn evaluate(
    text: &str,
    resolve: &mut dyn FnMut(&str) -> Result<Quantity, String>,
) -> Result<Quantity, String> {
    if text.len() > 512 {
        return Err("Dimension expression is too long.".into());
    }
    let mut parser = Parser {
        tokens: tokenize(text),
        index: 0,
        resolve,
    };
    let result = parser.sum(0)?;
    if parser.index != parser.tokens.len() {
        return Err(format!(
            "Unexpected dimension token: {}",
            parser.peek().unwrap_or("")
        ));
    }
    Ok(result)
}

/// `parseSketchValue`: the value in mm or degrees; dimensionless results are
/// accepted as the expected unit.
pub fn parse_sketch_value(
    text: &str,
    expected: ValueKind,
    resolve: &mut dyn FnMut(&str) -> Result<Quantity, String>,
) -> Result<f64, String> {
    let result = evaluate(text, resolve)?;
    if result.kind != ValueKind::Scalar && result.kind != expected {
        return Err(format!(
            "Expected a {} value, received {} units.",
            expected.name(),
            result.kind.name()
        ));
    }
    Ok(result.value)
}

pub fn valid_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 64
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `evaluateSketchVariables`: evaluates the whole set atomically, rejecting
/// invalid names, duplicates, unknown references and cycles.
pub fn evaluate_variables(
    variables: &[SketchVariable],
) -> Result<HashMap<String, Quantity>, String> {
    if variables.len() > 256 {
        return Err("A sketch supports at most 256 variables.".into());
    }
    let mut definitions = HashMap::new();
    for variable in variables {
        if !valid_variable_name(&variable.name) {
            return Err("Variable names require 1-64 letters, digits or underscores and cannot start with a digit.".into());
        }
        if definitions
            .insert(variable.name.as_str(), variable)
            .is_some()
        {
            return Err(format!("Duplicate sketch variable: #{}", variable.name));
        }
    }
    fn resolve(
        name: &str,
        definitions: &HashMap<&str, &SketchVariable>,
        values: &mut HashMap<String, Quantity>,
        active: &mut Vec<String>,
    ) -> Result<Quantity, String> {
        if let Some(value) = values.get(name) {
            return Ok(*value);
        }
        let definition = definitions
            .get(name)
            .ok_or_else(|| format!("Unknown sketch variable: #{name}"))?;
        if let Some(start) = active.iter().position(|item| item == name) {
            let cycle: Vec<String> = active[start..]
                .iter()
                .chain(std::iter::once(&name.to_string()))
                .map(|item| format!("#{item}"))
                .collect();
            return Err(format!("Cyclic sketch variables: {}", cycle.join(" -> ")));
        }
        if active.len() >= 64 {
            return Err("Sketch variable dependencies exceed 64 levels.".into());
        }
        active.push(name.to_string());
        let mut error = None;
        let result = {
            let mut lookup =
                |dependency: &str| match resolve(dependency, definitions, values, active) {
                    Ok(value) => Ok(value),
                    Err(message) => {
                        error.get_or_insert(message.clone());
                        Err(message)
                    }
                };
            parse_sketch_value(&definition.expression, definition.kind, &mut lookup)
        };
        active.pop();
        let value = result.map_err(|message| error.unwrap_or(message))?;
        let quantity = Quantity {
            value,
            kind: definition.kind,
        };
        values.insert(name.to_string(), quantity);
        Ok(quantity)
    }
    let mut values = HashMap::new();
    let mut active = Vec::new();
    for variable in variables {
        resolve(&variable.name, &definitions, &mut values, &mut active)?;
    }
    Ok(values)
}

/// Evaluates a dimension expression against a sketch's variables.
pub fn evaluate_dimension(
    text: &str,
    expected: ValueKind,
    variables: &[SketchVariable],
) -> Result<f64, String> {
    let values = evaluate_variables(variables)?;
    parse_sketch_value(text, expected, &mut |name| {
        values
            .get(name)
            .copied()
            .ok_or_else(|| format!("Unknown sketch variable: #{name}"))
    })
}

/// Text is a plain number (no expression worth storing).
pub fn is_plain_number(text: &str) -> bool {
    text.trim().parse::<f64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(text: &str, kind: ValueKind) -> Result<f64, String> {
        evaluate_dimension(text, kind, &[])
    }

    fn var(name: &str, kind: ValueKind, expression: &str) -> SketchVariable {
        SketchVariable {
            name: name.into(),
            kind,
            expression: expression.into(),
        }
    }

    #[test]
    fn arithmetic_precedence_and_parentheses() {
        assert_eq!(eval("1 + 2 * 3", ValueKind::Length), Ok(7.0));
        assert_eq!(eval("(1 + 2) * 3", ValueKind::Length), Ok(9.0));
        assert_eq!(eval("-4 / 2", ValueKind::Length), Ok(-2.0));
        assert_eq!(eval("1.5e1", ValueKind::Length), Ok(15.0));
        assert!((eval("pi", ValueKind::Angle).unwrap() - std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn units_convert_to_mm_and_degrees() {
        assert_eq!(eval("1 in", ValueKind::Length), Ok(25.4));
        assert_eq!(eval("2cm + 5mm", ValueKind::Length), Ok(25.0));
        assert_eq!(eval("90 deg", ValueKind::Angle), Ok(90.0));
        assert!((eval("pi rad", ValueKind::Angle).unwrap() - 180.0).abs() < 1e-9);
        assert_eq!(eval("10 MM", ValueKind::Length), Ok(10.0));
    }

    #[test]
    fn rejects_bad_expressions() {
        assert!(eval("10 mm", ValueKind::Angle).is_err());
        assert!(eval("10 mm + 5", ValueKind::Length).is_err());
        assert!(eval("1 / 0", ValueKind::Length).is_err());
        assert!(eval("(1 + 2", ValueKind::Length).is_err());
        assert!(eval("2 3", ValueKind::Length).is_err());
        assert!(eval("mm * mm", ValueKind::Length).is_err());
        assert!(eval("2mm * 3mm", ValueKind::Length).is_err());
        assert!(eval("#missing", ValueKind::Length).is_err());
        assert!(eval("", ValueKind::Length).is_err());
    }

    #[test]
    fn variables_resolve_in_any_order_with_units() {
        let variables = [
            var("total", ValueKind::Length, "#w * 2 + 1 in"),
            var("w", ValueKind::Length, "10 mm"),
            var("n", ValueKind::Scalar, "3"),
        ];
        let values = evaluate_variables(&variables).unwrap();
        assert!((values["total"].value - 45.4).abs() < 1e-9);
        assert_eq!(
            evaluate_dimension("#w * #n", ValueKind::Length, &variables),
            Ok(30.0)
        );
    }

    #[test]
    fn variables_reject_cycles_duplicates_and_bad_names() {
        let cycle = [
            var("a", ValueKind::Length, "#b"),
            var("b", ValueKind::Length, "#a"),
        ];
        assert!(evaluate_variables(&cycle).unwrap_err().contains("Cyclic"));
        let duplicate = [
            var("a", ValueKind::Length, "1"),
            var("a", ValueKind::Length, "2"),
        ];
        assert!(evaluate_variables(&duplicate).is_err());
        assert!(evaluate_variables(&[var("1a", ValueKind::Length, "1")]).is_err());
        assert!(evaluate_variables(&[var("a", ValueKind::Angle, "1 mm")]).is_err());
    }
}
