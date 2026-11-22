# Constraint System Plan

## Current State

Puppycad now has a node-based CAD runtime:

- `PCadState` stores graph nodes in a `Map`.
- Sketch entities are graph nodes (`sketchLine`, `sketchCornerRectangle`).
- `SketchNode` currently owns `dimensions` as an array.
- Setting a dimension currently edits entity coordinates immediately.

That is a good bridge, but it is not yet a real constraint system. The next goal is to make constraints authored data and derive solved geometry from them.

## Direction

Use first-class constraint nodes in the CAD graph.

Reasons:

- Constraints can participate in validation and delete cascade.
- Constraints can be selected, renamed, edited, disabled, deleted, and eventually shown in the feature/tree UI.
- Dimensions become a subtype of constraints instead of a special array on `SketchNode`.
- Solver input can be built from all sketch entity + constraint nodes for a sketch.

## Proposed Schema

```ts
export type SketchPointRef = {
	type: "point"
	entityId: string
	point: "p0" | "p1"
}

export type SketchEntityRef = {
	type: "entity"
	entityId: string
}

export type SketchConstraintNode = PCadNode & {
	readonly type: "sketchConstraint"
	readonly sketchId: string
	readonly constraint:
		| {
				readonly type: "coincident"
				readonly a: SketchPointRef
				readonly b: SketchPointRef
		  }
		| {
				readonly type: "horizontal"
				readonly entityId: string
		  }
		| {
				readonly type: "vertical"
				readonly entityId: string
		  }
		| {
				readonly type: "parallel"
				readonly aEntityId: string
				readonly bEntityId: string
		  }
		| {
				readonly type: "perpendicular"
				readonly aEntityId: string
				readonly bEntityId: string
		  }
		| {
				readonly type: "lineLength"
				readonly entityId: string
				readonly value: number
		  }
		| {
				readonly type: "pointDistance"
				readonly a: SketchPointRef
				readonly b: SketchPointRef
				readonly value: number
		  }
}
```

Open question: whether `sketchConstraint` should be one generic node type or separate node types like `sketchCoincidentConstraint`, `sketchLineLengthConstraint`, etc. I recommend one generic node initially; it keeps graph rewrites and UI lists simpler while the vocabulary is still changing.

## Rectangle Strategy

Rectangles are currently a primitive `sketchCornerRectangle` node. For a real solver, rectangles should eventually become generated lines + constraints:

- four `sketchLine` nodes
- coincident constraints at corners
- horizontal/vertical constraints
- maybe equal/opposite constraints depending on representation

Recommended staging:

1. Keep `sketchCornerRectangle` working for compatibility.
2. Introduce line-first solver support.
3. Later change the rectangle tool to emit lines + constraints instead of a rectangle primitive.
4. Keep a legacy adapter that materializes old rectangle nodes into equivalent solver primitives.

## Solver Model

Add a derived solve step, not persistent solved geometry.

```ts
export type SketchSolveResult = {
	readonly sketchId: string
	readonly entities: readonly SketchEntity[]
	readonly diagnostics: readonly SketchSolveDiagnostic[]
}

export type SketchSolveDiagnostic =
	| { readonly type: "underconstrained"; readonly entityIds: readonly string[] }
	| { readonly type: "overconstrained"; readonly constraintIds: readonly string[] }
	| { readonly type: "unsatisfied"; readonly constraintIds: readonly string[] }
```

The runtime should use solved/materialized geometry for profiles, drawing, and extrudes. Source graph stays authored.

## Implementation Phases

### Phase 1 — Constraint Nodes, No General Solver Yet

- Add `SketchConstraintNode` to schema.
- Add graph dependency support:
  - constraint depends on `sketchId`
  - constraint depends on referenced entity ids
- Add validation for referenced sketch + entities.
- Replace `SketchNode.dimensions` with constraint nodes in runtime, or support both during migration.
- Move current `setSketchDimension` behavior to create/update dimensional constraint nodes.
- Keep the current direct-coordinate update as a temporary compatibility solve.

Deliverable: dimensions are stored as graph nodes, existing UI behavior unchanged.

### Phase 2 — Minimal Line Solver

Support only line endpoints and these constraints:

- line length
- horizontal
- vertical
- coincident point-to-point

Use a small iterative numeric solver or deterministic special-case solver. I recommend deterministic special-case first because the entity vocabulary is tiny and it keeps failures understandable.

Deliverable: simple constrained sketches solve consistently without mutating source entities.

### Phase 3 — UI Integration

- Show constraint/dimension nodes in selection/edit panels.
- Allow deleting a constraint without deleting the entity.
- Highlight unsatisfied/overconstrained constraints.
- Keep dirty/editing state in `PartTreeState` or UI state, not CAD graph.

Deliverable: user can inspect and edit constraints as authored objects.

### Phase 4 — Rectangle Tool Migration

- Make rectangle creation emit four lines + constraints.
- Add adapter for old `sketchCornerRectangle` nodes.
- Update tests to assert solved/materialized geometry, not raw rectangle topology.

Deliverable: rectangle becomes solver-native.

### Phase 5 — Broader Constraint Vocabulary

Add only when needed:

- equal length
- point-on-line
- distance from point to line
- angle
- fixed point/entity
- symmetry/mirror
- tangent when arcs/circles exist

## First Implementation Slice

The smallest valuable PR should be:

1. Add `SketchConstraintNode` and ref types to `src/schema.ts`.
2. Add helpers in `src/pcad/sketch-constraints.ts`.
3. Extend `getNodeDependencies` and `validateNode`.
4. Add `CadEditor.setSketchLineLengthConstraint(...)` or generic `upsertSketchConstraint(...)`.
5. Keep old `setSketchDimension` API as a wrapper for compatibility.
6. Add tests for:
   - constraint node validation
   - delete cascade entity -> constraint
   - delete cascade sketch -> entity + constraint
   - updating line length still behaves like today

## Important Design Rule

Do not make the solver mutate source graph coordinates long-term. The source graph should store authored entities and authored constraints. Solved geometry is derived data.
