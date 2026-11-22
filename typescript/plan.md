# Node-Based CAD Runtime Plan

## Goal

Move the part editor runtime model toward an immutable node graph.

The core idea is:

- `PCadState` is a DAG-like runtime state.
- Nodes are stored in a `Map`.
- Root nodes are stored as a list of node ids.
- CAD operations are graph rewrite operations.
- UI/session state stays outside the CAD graph.
- Undo/redo is generic state history, not one-off CAD actions.

## Runtime State

Runtime state should focus on editor ergonomics, not persisted JSON shape.

```ts
export interface PCadState {
	readonly nodes: ReadonlyMap<string, PCadGraphNode>
	readonly rootNodeIds: readonly string[]
}
```

`rootNodeIds` should mean top-level visible/output nodes, not necessarily nodes with no dependencies.

For example, an `extrude` can be a root output even though it depends on a `profile`, which depends on a `sketch`.

## Node Style

Use plain `string` ids. Avoid aliases like `NodeId`, generic base types, or `NodeRef` until there is a concrete need.

References should be direct id fields:

```ts
profileId: string
sketchId: string
edgeId: string
sourceId: string
targetId: string
```

Runtime validation should check that those ids point to the expected node type.

## Core Schema Sketch

```ts
import type { Point2D, Vector3D } from "./types"

export type SketchPlane = "XY" | "YZ" | "XZ"
export type ReferencePlaneName = "Front" | "Top" | "Right"

export interface PCadNode {
	readonly id: string
	readonly type: string
	readonly name?: string
}

export interface ReferencePlaneNode extends PCadNode {
	readonly type: "referencePlane"
	readonly plane: SketchPlane
}

export interface Line {
	readonly id: string
	readonly type: "line"
	readonly p0: Point2D
	readonly p1: Point2D
}

export interface CornerRectangle {
	readonly id: string
	readonly type: "cornerRectangle"
	readonly p0: Point2D
	readonly p1: Point2D
}

export type SketchEntity = Line | CornerRectangle

export interface SketchLineNode extends PCadNode {
	readonly type: "sketchLine"
	readonly sketchId: string
	readonly p0: Point2D
	readonly p1: Point2D
}

export interface SketchCornerRectangleNode extends PCadNode {
	readonly type: "sketchCornerRectangle"
	readonly sketchId: string
	readonly p0: Point2D
	readonly p1: Point2D
}

export type SketchEntityNode = SketchLineNode | SketchCornerRectangleNode

export type SketchDimension =
	| {
			readonly id: string
			readonly type: "lineLength"
			readonly entityId: string
			readonly value: number
	  }
	| {
			readonly id: string
			readonly type: "rectangleWidth"
			readonly entityId: string
			readonly value: number
	  }
	| {
			readonly id: string
			readonly type: "rectangleHeight"
			readonly entityId: string
			readonly value: number
	  }

export interface SketchNode extends PCadNode {
	readonly type: "sketch"
	readonly targetId: string
	readonly dimensions: readonly SketchDimension[]
}

export type ExtrudeOperation = "newBody" | "join" | "cut"

export interface ExtrudeNode extends PCadNode {
	readonly type: "extrude"
	readonly sketchId: string
	readonly profileId: string
	readonly operation: ExtrudeOperation
	readonly depth: number
}

export interface FaceNode extends PCadNode {
	readonly type: "face"
	readonly sourceId: string
	readonly faceId: string
}

export interface EdgeNode extends PCadNode {
	readonly type: "edge"
	readonly sourceId: string
	readonly edgeId: string
}

export interface ChamferNode extends PCadNode {
	readonly type: "chamfer"
	readonly edgeId: string
	readonly d1: number
	readonly d2?: number
}

export type PCadGraphNode =
	| ReferencePlaneNode
	| SketchNode
	| SketchEntityNode
	| ExtrudeNode
	| FaceNode
	| EdgeNode
	| ChamferNode

export interface SolidVertex {
	readonly id: string
	readonly position: Vector3D
}

export interface SolidEdge {
	readonly id: string
	readonly vertexIds: readonly string[]
}

export interface SolidFace {
	readonly id: string
	readonly edgeIds: readonly string[]
}

export interface Solid {
	readonly id: string
	readonly sourceId: string
	readonly vertices: readonly SolidVertex[]
	readonly edges: readonly SolidEdge[]
	readonly faces: readonly SolidFace[]
}
```

## Derived Data

Keep authored CAD state and derived geometry separate.

The source graph should not store materialized sketch topology such as:

- vertices
- loops
- profiles

Those should be computed from sketch entity graph nodes and dimensions when needed.

Operations that consume sketch profiles should reference derived sketch regions by stable profile id:

```ts
export interface ExtrudeNode extends PCadNode {
	readonly type: "extrude"
	readonly sketchId: string
	readonly profileId: string
	readonly operation: ExtrudeOperation
	readonly depth: number
}
```

Generated faces, edges, and solids should also be treated as computed data unless a persistent cache is explicitly needed.

## Graph Rewrites

The primitive mutation model should be graph rewrites.

```ts
export type PCadGraphRewrite =
	| {
			readonly type: "addNode"
			readonly node: PCadGraphNode
			readonly root?: boolean
	  }
	| {
			readonly type: "replaceNode"
			readonly node: PCadGraphNode
	  }
	| {
			readonly type: "removeNodes"
			readonly nodeIds: readonly string[]
	  }
	| {
			readonly type: "setRootNodes"
			readonly rootNodeIds: readonly string[]
	  }
```

Higher-level CAD operations compile to one or more rewrites.

```ts
export function applyGraphRewrite(state: PCadState, rewrite: PCadGraphRewrite): PCadState {
	switch (rewrite.type) {
		case "addNode": {
			const nodes = new Map(state.nodes).set(rewrite.node.id, rewrite.node)
			const rootNodeIds = rewrite.root && !state.rootNodeIds.includes(rewrite.node.id) ? [...state.rootNodeIds, rewrite.node.id] : state.rootNodeIds
			return { nodes, rootNodeIds }
		}
		case "replaceNode":
			if (!state.nodes.has(rewrite.node.id)) {
				return state
			}
			return {
				...state,
				nodes: new Map(state.nodes).set(rewrite.node.id, rewrite.node)
			}
		case "removeNodes": {
			const removedIds = new Set(rewrite.nodeIds)
			const nodes = new Map(state.nodes)
			for (const id of removedIds) {
				nodes.delete(id)
			}
			return {
				nodes,
				rootNodeIds: state.rootNodeIds.filter((id) => !removedIds.has(id))
			}
		}
		case "setRootNodes":
			return {
				...state,
				rootNodeIds: [...rewrite.rootNodeIds]
			}
	}
}

export function applyGraphRewrites(state: PCadState, rewrites: readonly PCadGraphRewrite[]): PCadState {
	return rewrites.reduce((nextState, rewrite) => applyGraphRewrite(nextState, rewrite), state)
}
```

## Dependencies

Dependencies can be derived from each node.

```ts
export function getNodeDependencies(node: PCadGraphNode): readonly string[] {
	switch (node.type) {
		case "referencePlane":
			return []
		case "sketch":
			return [node.targetId]
		case "sketchLine":
		case "sketchCornerRectangle":
			return [node.sketchId]
		case "extrude":
			return [node.sketchId]
		case "face":
		case "edge":
			return [node.sourceId]
		case "chamfer":
			return [node.edgeId]
	}
}
```

Delete cascade should be implemented as reverse graph traversal from dependency edges.

## Strict CAD Operations

Current part editor operations that strictly modify CAD state are:

- add sketch entity
- clear sketch geometry
- set sketch dimension
- extrude sketch profile
- set extrude depth
- chamfer edge
- set chamfer distance
- delete node cascade

Do not model undo as a sketch-specific CAD operation. Undo/redo should be generic immutable history.

## UI Or Session State

These should stay outside the CAD DAG:

- start sketch mode
- finish sketch mode
- exit sketch mode
- selected sketch/face/edge/corner
- active sketch tool
- pending line start
- pending rectangle start
- hover point
- camera orbit/pan/zoom
- reference plane visibility
- sketch visibility

`finishSketch` is currently a CAD action because the old schema stores `dirty`, `vertices`, `loops`, and `profiles` on the sketch. In the DAG model, this should become UI/session state unless we later need a source-level `status`.

## CadEditor Runtime Wrapper

Use a class for ergonomic editing, but keep schema and rewrites as plain data/functions.

The class should own:

- current immutable `PCadState`
- undo/redo history
- validation
- high-level CAD operations

```ts
export class CadEditor {
	private past: PCadState[] = []
	private present: PCadState
	private future: PCadState[] = []

	public constructor(initialState = createEmptyPCadState()) {
		this.present = initialState
	}

	public getState(): PCadState {
		return this.present
	}

	public undo(): void {
		const previous = this.past.at(-1)
		if (!previous) {
			return
		}
		this.future = [this.present, ...this.future]
		this.present = previous
		this.past = this.past.slice(0, -1)
	}

	public redo(): void {
		const [next, ...rest] = this.future
		if (!next) {
			return
		}
		this.past = [...this.past, this.present]
		this.present = next
		this.future = rest
	}

	public addSketchEntity(sketchId: string, entity: SketchEntity): SketchEntityNode {
		this.getSketchOrThrow(sketchId)
		const node = sketchEntityToNode(sketchId, entity)
		this.commit({ type: "addNode", node })
		return node
	}

	public clearSketch(sketchId: string): SketchNode {
		const sketch = this.getSketchOrThrow(sketchId)
		const entityNodeIds = getSketchEntityNodes(this.present, sketch.id).map((node) => node.id)
		const nextSketch: SketchNode = {
			...sketch,
			dimensions: []
		}
		this.commitMany([
			{ type: "removeNodes", nodeIds: entityNodeIds },
			{ type: "replaceNode", node: nextSketch }
		])
		return nextSketch
	}

	public setSketchDimension(sketchId: string, dimension: SketchDimension): SketchNode {
		const sketch = this.getSketchOrThrow(sketchId)
		const dimensions = sketch.dimensions.filter((item) => !(item.entityId === dimension.entityId && item.type === dimension.type))
		const nextSketch: SketchNode = {
			...sketch,
			dimensions: [...dimensions, dimension]
		}
		this.commit({ type: "replaceNode", node: nextSketch })
		return nextSketch
	}

	public extrudeSketchProfile(args: {
		id: string
		name?: string
		sketchId: string
		profileId: string
		operation: ExtrudeOperation
		depth: number
	}): ExtrudeNode {
		const sketch = this.getSketchOrThrow(args.sketchId)
		if (!Number.isFinite(args.depth) || args.depth <= 0) {
			throw new Error("Extrude depth must be greater than zero.")
		}
		if (!["newBody", "join", "cut"].includes(args.operation)) {
			throw new Error("Extrude operation must be newBody, join, or cut.")
		}

		const extrude: ExtrudeNode = {
			id: args.id,
			type: "extrude",
			name: args.name,
			sketchId: sketch.id,
			profileId: args.profileId,
			operation: args.operation,
			depth: args.depth
		}

		this.commit({ type: "addNode", node: extrude, root: true })

		return extrude
	}

	public setExtrudeDepth(extrudeId: string, depth: number): ExtrudeNode {
		const extrude = this.getExtrudeOrThrow(extrudeId)
		if (!Number.isFinite(depth) || depth <= 0) {
			throw new Error("Extrude depth must be greater than zero.")
		}

		const nextExtrude: ExtrudeNode = {
			...extrude,
			depth
		}
		this.commit({ type: "replaceNode", node: nextExtrude })
		return nextExtrude
	}

	public chamferEdge(args: {
		id: string
		name?: string
		edgeId: string
		d1: number
		d2?: number
	}): ChamferNode {
		const edge = this.getEdgeOrThrow(args.edgeId)
		if (!Number.isFinite(args.d1) || args.d1 <= 0) {
			throw new Error("Chamfer distance must be greater than zero.")
		}

		const chamfer: ChamferNode = {
			id: args.id,
			type: "chamfer",
			name: args.name,
			edgeId: edge.id,
			d1: args.d1,
			...(args.d2 === undefined ? {} : { d2: args.d2 })
		}

		this.commit({ type: "addNode", node: chamfer, root: true })
		return chamfer
	}

	public setChamferDistance(chamferId: string, d1: number, d2?: number): ChamferNode {
		const chamfer = this.getChamferOrThrow(chamferId)
		if (!Number.isFinite(d1) || d1 <= 0) {
			throw new Error("Chamfer distance must be greater than zero.")
		}

		const nextChamfer: ChamferNode = {
			...chamfer,
			d1,
			...(d2 === undefined ? {} : { d2 })
		}
		this.commit({ type: "replaceNode", node: nextChamfer })
		return nextChamfer
	}

	public deleteNodeCascade(nodeId: string): void {
		const deletedIds = collectDependentNodeIds(this.present, [nodeId])
		this.commit({ type: "removeNodes", nodeIds: [...deletedIds] })
	}

	private commit(rewrite: PCadGraphRewrite): void {
		this.commitMany([rewrite])
	}

	private commitMany(rewrites: readonly PCadGraphRewrite[]): void {
		const next = applyGraphRewrites(this.present, rewrites)
		if (next === this.present) {
			return
		}
		this.past = [...this.past, this.present]
		this.present = next
		this.future = []
	}

	private getSketchOrThrow(id: string): SketchNode {
		const node = this.present.nodes.get(id)
		if (!node || node.type !== "sketch") {
			throw new Error(`Sketch "${id}" does not exist.`)
		}
		return node
	}

	private getExtrudeOrThrow(id: string): ExtrudeNode {
		const node = this.present.nodes.get(id)
		if (!node || node.type !== "extrude") {
			throw new Error(`Extrude "${id}" does not exist.`)
		}
		return node
	}

	private getEdgeOrThrow(id: string): EdgeNode {
		const node = this.present.nodes.get(id)
		if (!node || node.type !== "edge") {
			throw new Error(`Edge "${id}" does not exist.`)
		}
		return node
	}

	private getChamferOrThrow(id: string): ChamferNode {
		const node = this.present.nodes.get(id)
		if (!node || node.type !== "chamfer") {
			throw new Error(`Chamfer "${id}" does not exist.`)
		}
		return node
	}
}
```

## Migration Notes

The existing part editor still expects:

```ts
features: PartFeature[]
```

The migration should be staged:

1. Add the DAG runtime model next to the current schema.
2. Add adapters between legacy `features` state and `PCadState`.
3. Move core operations from `applyPartAction` toward `CadEditor` methods.
4. Keep UI behavior unchanged while swapping implementation internals.
5. Remove persisted derived sketch topology once consumers use materialization helpers.
6. Later, decide whether persisted files should use an array of nodes or another JSON shape.
