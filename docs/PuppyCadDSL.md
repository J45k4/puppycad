# PuppyCAD DSL Specification v0.1

This document defines the **PuppyCAD DSL** (“PuppyCAD language”) intended to be:
- **Source-of-truth text** (humans + AI edit directly)
- **Deterministic + diff-friendly**
- **AI-modifiable** (structural edits, small patches)
- **Intent-based attachments** (no fragile topology IDs in source)
- Easy to parse, validate, and pretty-print

> Design principle: the DSL expresses **intent** and **relationships**. The engine resolves those to geometry and kernel topology per rebuild.

---

## 0. File + encoding

- Files are UTF-8 text.
- Recommended extension: `.pcad`
- Newlines: `\n` preferred; accept `\r\n`.
- The language is **declarative**: source order does **not** define build semantics; dependencies do.

---

## 1. Lexical structure

### 1.1 Whitespace
- Spaces, tabs, and newlines are whitespace.
- Whitespace is insignificant except to separate tokens.

### 1.2 Comments
- Line comment: `// ...` until end of line
- Block comment: `/* ... */` (non-nested in v0.1)

### 1.3 Identifiers
- `IDENT = [A-Za-z_][A-Za-z0-9_]*`
- Case-sensitive.

### 1.4 Keywords
Reserved keywords (cannot be used as identifiers):
- `solid`, `feature`, `let`, `true`, `false`, `null`

### 1.5 Literals

**Numbers**
- Integer: `123`
- Float: `12.34`
- Float exponent: `1e-3`, `2.0E6`
- Unary minus is handled by expressions, not by the lexer.

**Strings**
- Double quoted: `"text"`
- Escapes: `\"`, `\\`, `\n`, `\t`, `\r`
- (Optional for v0.1) Unicode escapes: `\uXXXX`

**Booleans**
- `true`, `false`

**Null**
- `null`

---

## 2. Top-level structure

A file is a list of declarations:
- `solid` declarations create **geometry bodies**
- `feature` declarations create **operations** that modify or derive geometry (hole, chamfer, fillet, boolean, transform, etc.)

Each declaration has:
- a **kind** (`solid` or `feature`)
- a **node id** (unique per file)
- an **op** (e.g. `box`, `hole`, `chamfer`)
- a **block** of entries (fields + optional `let`)

---

## 3. Grammar (EBNF)

### 3.1 File
```ebnf
file        := { declaration } EOF ;
```

### 3.2 Declarations
```ebnf
declaration := solid_decl | feature_decl ;

solid_decl  := "solid" IDENT "=" IDENT block ;
feature_decl:= "feature" IDENT "=" IDENT block ;
```

### 3.3 Block + entries
```ebnf
block       := "{" { entry } "}" ;

entry       := let_stmt | field_stmt ;

let_stmt    := "let" IDENT "=" expr ";" ;

field_stmt  := IDENT ":" expr ";" ;
```

Notes:
- `;` is **required**. This makes parsing and patching deterministic.
- `let` bindings are **local to the block** and are intended to reduce repetition.

---

## 4. Expressions

### 4.1 Expression grammar + precedence
```ebnf
expr        := logic_or ;

logic_or    := logic_and { "||" logic_and } ;
logic_and   := equality { "&&" equality } ;

equality    := compare { ("==" | "!=") compare } ;
compare     := add { ("<" | "<=" | ">" | ">=") add } ;

add         := mul { ("+" | "-") mul } ;
mul         := unary { ("*" | "/" | "%") unary } ;

unary       := ("-" | "!") unary | primary ;

primary     := number
            | string
            | boolean
            | "null"
            | vector
            | object
            | reference
            | IDENT
            | call
            | "(" expr ")" ;
```

### 4.2 Literals in expressions

**Vectors**
```ebnf
vector      := "[" expr "," expr "," expr "]" ;
```
- v0.1 defines `vec3` only.

**Objects** (for intent queries and structured parameters)
```ebnf
object      := "{" { object_field } "}" ;
object_field:= IDENT ":" expr ";" ;
```

### 4.3 References
References are dotted paths:
```ebnf
reference   := IDENT "." IDENT { "." IDENT } ;
```

Common patterns:
- `body.top` (intent anchor)
- `body.w` (read a field from another node)
- `arm_base.mount_surface` (role/anchor)

### 4.4 Function calls (builtins)
```ebnf
call        := IDENT "(" [ arg_list ] ")" ;
arg_list    := expr { "," expr } ;
```

Built-in functions (v0.1):
- `min(a,b)`, `max(a,b)`
- `abs(x)`
- `clamp(x, lo, hi)`
- `sqrt(x)`
- `sin(x)`, `cos(x)`, `tan(x)` (radians)
- `deg(x)` (degrees → radians)
- `rad(x)` (identity; optional)
- `vec3(x,y,z)` (returns vec3; optional convenience)

Built-in constants (v0.1):
- `pi`, `tau`

---

## 5. Name resolution rules (deterministic)

When evaluating an `expr` inside a block:

1. **Local `let` bindings** defined earlier in the same block
2. **Fields** defined earlier in the same block (fields are symbols too)
3. **Dotted references**:
   - `node.field` resolves to another node’s evaluated field value
   - `node.anchor` resolves as an **intent anchor** only in contexts expecting a `TargetRef` (see §7)
4. **Builtins**:
   - functions (`min`, `max`, …)
   - constants (`pi`, `tau`)
5. Otherwise: `unknown_identifier` error

### 5.1 Forward references
- Within a block: `let` and field symbols must be defined **before use**.
- Across nodes: `node.field` may refer to nodes declared anywhere in the file; dependency graph handles ordering.

### 5.2 Cycles
- Cycles in local `let`/field evaluation are errors.
- Cycles in node dependencies are errors (`dependency_cycle`).

---

## 6. Type system (lightweight, v0.1)

Types:
- `number`
- `bool`
- `string`
- `null`
- `vec3`
- `object`
- `targetref` (special; see §7)

Type checking is **schema-driven per op** (engine defines expected fields and types).

Errors:
- `type_mismatch`
- `unknown_field` (if strict mode enabled; see §10)
- `missing_field` (if strict mode enabled)

---

## 7. Intent-based attachment model (TargetRef)

### 7.1 Where TargetRef is used
Certain fields are defined by op schemas to expect a `TargetRef`, e.g.:
- `hole.target`
- `chamfer.target`
- `fillet.target`

### 7.2 TargetRef forms (v0.1)

A `TargetRef` can be:

**A) Anchor reference (core v0.1)**
- Syntax: `node.anchor` (exactly 2 segments)
- Example: `body.top`, `body.front`, `chassis.mount_surface`

Interpretation:
- `node` must exist and evaluate to a solid.
- `anchor` is resolved by the engine using:
  - primitive anchors (e.g. `top/bottom/left/right/front/back` for `box`)
  - op-provided anchors (optional)
  - future: user-defined roles

**B) Query object (optional but recommended in v0.1)**
- Syntax: object literal with required discriminator `kind`

Example:
```pcad
target: {
  kind: "face";
  of: body;
  normal: [0, 0, 1];
  nearest_to: [0, 0, 30];
};
```

Query kinds (v0.1):
- `kind: "face"`
  - required: `of` (node id or reference to solid node)
  - optional: `normal: vec3`
  - optional: `nearest_to: vec3`
  - optional: `role: string`

Resolution rules (deterministic):
1. Collect candidate faces of `of`
2. If `role` provided and roles exist: match role first
3. If `normal` provided: minimize angular difference to `normal`
4. If `nearest_to` provided: minimize distance from face centroid/closest point to `nearest_to`
5. If multiple candidates tie within tolerance → `ambiguous_target`
6. If no candidate → `target_not_found`

### 7.3 Important: DSL never stores topology IDs
- The engine may internally assign kernel topology IDs per rebuild.
- The DSL expresses **intent**, not topology identity.

---

## 8. Dependency graph semantics

### 8.1 Dependency extraction
A node depends on another node if:
- any expression inside its block references `other.field` or `other.anchor`
- any `TargetRef` references `other` (anchor or query object)

### 8.2 Build order
- Build order is computed by **topological sort** of dependencies.
- If two nodes are independent, tie-breaker for build order:
  1) source order in file (stable)
  2) then lexicographic by node id (optional)

### 8.3 Cycles
- If cycle detected, return `dependency_cycle` with nodes involved.

---

## 9. Canonical formatting (pretty-printer contract)

To enable clean diffs and stable AI edits, PuppyCAD defines a canonical formatter.

### 9.1 Layout
- Indent: 2 spaces.
- Braces on same line:
  - `solid body = box {`
- Each entry ends with `;`
- Blank line between declarations.

### 9.2 Ordering
Inside a block, canonical formatter should output:
1. all `let` statements first (source order is acceptable)
2. then all fields in **schema-defined order** if schema is known
3. otherwise alphabetical by field name

### 9.3 Normalization
- Vectors: `[a, b, c]` with spaces after commas
- Objects: one field per line, each ending with `;`

---

## 10. Validation modes

### 10.1 Parser validation (always)
- syntax correctness
- reserved keyword misuse
- duplicate node ids

### 10.2 Semantic validation (engine/language server)
Two levels:

**Lenient (default)**
- unknown fields allowed (stored but warned) for forward compatibility

**Strict (optional)**
- unknown field errors
- missing required fields errors

---

## 11. Standard error format (machine-readable)

All tools (parser, engine, LSP) should emit errors as structured JSON:

```json
{
  "level": "error",
  "code": "syntax_error",
  "message": "Human-readable summary",
  "span": {
    "file": "model.pcad",
    "start": { "line": 12, "col": 5 },
    "end":   { "line": 12, "col": 18 }
  },
  "node": "optional_node_id",
  "details": { "optional": "machine fields for AI" }
}
```

Common `code` values:
- `syntax_error`
- `unknown_identifier`
- `type_mismatch`
- `duplicate_id`
- `dependency_cycle`
- `target_not_found`
- `ambiguous_target`
- `unknown_field`
- `missing_field`

---

## 12. Minimal op schema interface (engine-side contract)

The DSL is generic. The engine defines ops via a schema with:
- required fields (name + type)
- optional fields
- which fields are `TargetRef`
- result kind: `solid` | `feature` (assemblies later)

Illustrative example:
```json
{
  "op": "box",
  "kind": "solid",
  "fields": {
    "w": { "type": "number", "required": true },
    "h": { "type": "number", "required": true },
    "d": { "type": "number", "required": true }
  },
  "anchors": ["top", "bottom", "left", "right", "front", "back"]
}
```

---

## 13. Examples (canonical)

### 13.1 Box + centered hole using expressions
```pcad
solid body = box {
  w: 100;
  h: 50;
  d: 30;
}

feature hole1 = hole {
  let cx = body.w / 2;
  let cy = body.h / 2;

  target: body.top;
  x: cx;
  y: cy;
  d: 5;
}
```

### 13.2 Chamfer with query target
```pcad
solid body = box {
  w: 100;
  h: 50;
  d: 30;
}

feature ch1 = chamfer {
  target: { kind: "face"; of: body; normal: [0, 0, 1]; };
  dist: 2;
}
```

---

## 14. Implementation checklist (recommended order)

1. Lexer + parser for the v0.1 grammar (with spans)
2. AST structures:
   - `File { decls: Vec<Decl> }`
   - `Decl { kind, id, op, entries }`
   - `Entry::Let(name, expr)` / `Entry::Field(name, expr)`
   - `Expr` nodes (binary/unary/call/literal/ref/object/vector)
3. Pretty-printer implementing §9 canonical rules
4. Name resolution + expression evaluator
   - local symbol table (`let` + fields)
   - cross-node references via dependency graph
   - cycle detection
5. Dependency graph topo sort
6. TargetRef parsing + semantic representation
7. Error output exactly per §11

---

## 15. Notes for future versions (non-normative)

Potential v0.2+ additions:
- units (e.g. `mm`, `deg`) with explicit typing rules
- assemblies and constraints (`mate` / `joint`)
- richer target queries and role tagging
- partial rebuild caching and stable semantic anchors
- module/import system and versioning
