import { fixedEntityRelations } from "../sketch-fix"
import { sampleEllipticArc, normalizeEllipticArc } from "../sketch-elliptic-arc"
import { evaluateSketchVariables, renameSketchVariable, type SketchVariable } from "../sketch-variables"
import { sampleSpline, normalizeSpline } from "../sketch-spline"
import { translateSketchSelection, scaleSketchSelection, rotateSketchSelection } from "../sketch-translate"
import { chamferSketchCorner, setChamferMode } from "../sketch-chamfer"
import { sketchSnap, type SketchSnap } from "../sketch-snap"
import { parseSketchValue, type SketchValueKind } from "../sketch-value"
import { isMeasuredDimension, measureSketchDimension } from "../sketch-dimensions"
import { sketchEntityInBox } from "../sketch-selection"
import { threePointEllipse } from "../sketch-ellipse"
import { regularPolygon, setPolygonSideCount } from "../sketch-polygon"
import { tangentArc, sketchEndpointDirection } from "../sketch-tangent-arc"
import { circularPatternDragAngle, circularPatternCenter, setCircularPatternCenter, createCircularSketchPattern, resizeCircularSketchPattern } from "../sketch-circular-pattern"
import { patternLayout, createLinearSketchPattern, resizeLinearSketchPattern } from "../sketch-pattern"
import { sketchRelationEntityIds } from "../sketch-relations"
import { filletSketchCorner } from "../sketch-fillet"
import { centerRectangle, threePointRectangle, threePointCenterRectangle } from "../sketch-rectangle"
import { mirrorSketchEntity } from "../sketch-mirror"
import { threePointSlot } from "../sketch-slot"
import { orderOffsetChain, offsetLineChain } from "../sketch-offset-chain"
import { offsetSketchEntity } from "../sketch-offset"
import { editSketchCurve, type SketchEditOperation } from "../sketch-edit"
import { arcPoint, arcPoints, threePointCircle, threePointArc, centerPointArc, entityAnchorNames, entityAnchorPoint } from "../sketch-curves"
import type { Sketch, SketchEntity } from "../schema"
import type { Point2D } from "../types"
import { normalizeSketchRelations, resolveSketchDimensionExpressions, solveSketch, type SketchAnchor, type SketchRelation, type SketchSolveResult } from "../sketch-solver"
import { materializeSketch } from "../cad/sketch"
import { primitivePoints } from "../sketch-primitives"
import { requireValue } from "../required"

const selectionFilters = ["All geometry", "Construction", "Non-construction", "Points", "Lines", "Curves"] as const

type Tool =
	| "Fit spline"
	| "Control spline"
	| "Ellipse"
	| "Polygon"
	| "Point"
	| "Select"
	| "Line"
	| "Rectangle"
	| "Center rectangle"
	| "3-point rectangle"
	| "3-point center rectangle"
	| "Circle"
	| "Diameter circle"
	| "3-point circle"
	| "3-point arc"
	| "Center arc"
	| "Tangent arc"
	| "Slot"
	| "Dimension"
	| SketchEditOperation
const constraintLabel = (type: SketchRelation["type"]): string =>
	type === "smoothJoin" ? "Tangent join" : type === "pointOnCurve" ? "Point on curve" : type === "sameEllipse" ? "Shared ellipse" : type
const NS = "http://www.w3.org/2000/svg"
const need = <T>(value: T | null | undefined, message: string): T => {
	if (value == null) throw Error(message)
	return value
}
const copy = <T>(value: T): T => structuredClone(value)

/** A transactional plane workspace: Finish commits one sketch; Cancel never changes its source. */
export class SketchWorkspace {
	readonly root = document.createElement("section")
	private sketch: Sketch
	private readonly svg = document.createElementNS(NS, "svg")
	private readonly properties = document.createElement("aside")
	private readonly status = document.createElement("p")
	private readonly tools = document.createElement("nav")
	private tool: Tool = "Select"
	private splinePoints: Point2D[] = []
	private construction = false
	private clockwiseArc = false
	private chamferAngleMode = false
	private chamferAngle = 45
	private rotationAngle = 90
	private rotationCenter = { x: 0, y: 0 }
	private scaleFactor = 2
	private scaleCenter = { x: 0, y: 0 }
	private moveX = 10
	private moveY = 0
	private chamferFirst = 5
	private chamferSecond = 5
	private filletRadius = 5
	private snapGrid = true
	private snapGeometry = true
	private autoConstraints = true
	private hoverSnap: SketchSnap | null = null
	private showConstraints = false
	private start: Point2D | null = null
	private tangentStart: SketchAnchor | null = null
	private polygonSides = 6
	private circumscribedPolygon = false
	private secondPoint: Point2D | null = null
	private splitCircleStart: { id: string; point: Point2D } | null = null
	private cursor: Point2D | null = null
	private marquee: { start: Point2D; end: Point2D; additive: boolean; initial: string[]; initialAnchors: SketchAnchor[]; initialRelation: string | null } | null = null
	private selected: string[] = []
	private anchors: SketchAnchor[] = []
	private selectionFilter: (typeof selectionFilters)[number] = "All geometry"
	private selectedRelation: string | null = null
	private undo: Sketch[] = []
	private redo: Sketch[] = []
	private drag: {
		groupScale?: { center: Point2D; vector: Point2D; applied: number; selected: string[]; removed: string[] }
		groupMove?: { start: Point2D; selected: string[]; removed: string[] }
		groupRotate?: { center: Point2D; initial: number; last: number; radius: number; value: number; applied: number; selected: string[]; removed: string[] }
		anchor?: SketchAnchor
		label?: string
		patternStep?: { id: string; axis: "column" | "row" }
		patternCenter?: string
		patternAngle?: { id: string; last: number; value: number }
		before: Sketch
		pan?: Point2D
		center?: Point2D
	} | null = null
	private view = { x: 0, y: 0, scale: 3 }
	private width = 1000
	private height = 700
	private observer?: ResizeObserver
	private resizeFrame?: number
	private initialFitPending = true
	private showSupport = true
	private disposed = false
	private toolPalette?: HTMLElement
	private result: SketchSolveResult
	constructor(
		sketch: Sketch,
		private readonly finish: (sketch: Sketch) => void,
		private readonly cancel: () => void,
		private readonly supportEdges: Point2D[][] = [],
		private readonly supportError?: string
	) {
		this.sketch = copy(sketch)
		this.sketch.relations = resolveSketchDimensionExpressions(this.sketch.relations, this.sketch.variables) ?? []
		this.result = solveSketch(this.sketch.entities, this.sketch.relations)
		this.root.setAttribute("aria-label", "Sketch workspace")
		this.root.setAttribute("data-sketch-workspace", "")
		this.root.style.cssText =
			"position:relative;width:100%;height:100%;min-height:0;min-width:0;background:#fff;display:grid;grid-template-columns:min(260px,40vw) minmax(0,1fr);grid-template-rows:auto minmax(0,1fr) auto;color:#172033;font:14px system-ui"
		this.tools.setAttribute("aria-label", "Sketch tools")
		this.tools.style.cssText = "grid-column:1/-1;display:flex;gap:5px;flex-wrap:wrap;padding:10px;border-bottom:1px solid #ccd4df;background:#f8fafc"
		this.root.append(this.tools, this.properties, this.svg, this.status)
		this.properties.style.cssText = "overflow:auto;padding:14px;border-right:1px solid #ccd4df"
		this.svg.style.cssText = "width:100%;height:100%;min-height:0;min-width:0;touch-action:none;outline:none;background:#fff"
		this.svg.setAttribute("role", "application")
		this.svg.setAttribute("aria-label", "Sketch drawing canvas")
		this.svg.setAttribute("tabindex", "0")
		this.status.setAttribute("role", "status")
		this.status.style.cssText = "grid-column:1/-1;margin:0;padding:9px;border-top:1px solid #ccd4df;background:#f8fafc"
		this.svg.addEventListener("pointerdown", (e) => this.pointerDown(e))
		this.svg.addEventListener("pointermove", (e) => this.pointerMove(e))
		this.svg.addEventListener("pointerleave", () => {
			if (!this.hoverSnap) return
			this.hoverSnap = null
			this.draw()
		})
		this.svg.addEventListener("pointerup", () => this.pointerUp())
		this.svg.addEventListener("pointercancel", () => this.cancelDrag())
		this.svg.addEventListener("contextmenu", (e) => e.preventDefault())
		this.svg.addEventListener(
			"wheel",
			(e) => {
				e.preventDefault()
				const before = this.world(e.clientX, e.clientY, false)
				this.view.scale = Math.max(0.05, Math.min(200, this.view.scale * Math.exp(-e.deltaY * 0.001)))
				const after = this.world(e.clientX, e.clientY, false)
				this.view.x += before.x - after.x
				this.view.y += before.y - after.y
				this.draw()
			},
			{ passive: false }
		)
		this.root.addEventListener(
			"pointerdown",
			(event) => {
				for (const menu of Array.from(this.tools.querySelectorAll<HTMLDetailsElement>("details[open]"))) {
					if (event.target instanceof window.Node && !menu.contains(event.target)) menu.open = false
				}
			},
			true
		)
		this.root.addEventListener("keydown", (e) => this.key(e))
		if (typeof ResizeObserver !== "undefined") {
			this.observer = new ResizeObserver(() => {
				if (this.resizeFrame !== undefined) cancelAnimationFrame(this.resizeFrame)
				this.resizeFrame = requestAnimationFrame(() => {
					this.resizeFrame = undefined
					if (this.initialFitPending) this.fit()
					else this.draw()
				})
			})
			this.observer.observe(this.svg)
		}
		this.renderTools()
		this.renderProperties()
		this.fit()
	}
	private button(parent: HTMLElement, label: string, action: () => void): HTMLButtonElement {
		const b = document.createElement("button")
		b.type = "button"
		b.textContent = label
		b.style.cssText = "padding:6px 9px;border:1px solid #cbd5e1;border-radius:4px;background:white;cursor:pointer"
		b.onclick = action
		parent.append(b)
		return b
	}
	private closeToolPalette() {
		this.toolPalette?.remove()
		this.toolPalette = undefined
	}
	private openToolPalette() {
		this.closeToolPalette()
		const actions = Array.from(this.tools.querySelectorAll("button")).filter(
			(button) => !button.disabled && !["Search tools", "Finish sketch", "Cancel sketch"].includes(button.textContent ?? "")
		)
		const panel = document.createElement("div")
		this.toolPalette = panel
		panel.setAttribute("aria-label", "Tool search")
		panel.style.cssText =
			"position:absolute;z-index:5;top:55px;left:50%;transform:translateX(-50%);width:min(400px,90vw);padding:12px;background:white;border:1px solid #94a3b8;border-radius:8px;box-shadow:0 8px 30px #17203333"
		const search = document.createElement("input")
		search.type = "search"
		search.setAttribute("aria-label", "Search sketch tools")
		search.setAttribute("role", "combobox")
		search.setAttribute("aria-expanded", "true")
		search.setAttribute("aria-autocomplete", "list")
		search.setAttribute("aria-controls", "sketch-tool-results")
		search.placeholder = "Search tools…"
		search.style.cssText = "box-sizing:border-box;width:100%;padding:8px"
		const list = document.createElement("div")
		list.id = "sketch-tool-results"
		list.setAttribute("role", "listbox")
		list.setAttribute("aria-label", "Matching sketch tools")
		list.style.cssText = "max-height:320px;overflow:auto;margin-top:8px"
		let index = 0
		let matches = actions
		const activate = (action: HTMLButtonElement | undefined) => {
			if (!action || this.disposed || this.toolPalette !== panel || !action.isConnected) return
			this.closeToolPalette()
			this.svg.focus()
			action.click()
		}
		const render = () => {
			list.replaceChildren()
			search.removeAttribute("aria-activedescendant")
			matches.forEach((action, i) => {
				const option = this.button(list, action.textContent ?? "", () => activate(action))
				option.id = `sketch-tool-option-${i}`
				option.setAttribute("role", "option")
				option.setAttribute("aria-selected", String(i === index))
				option.tabIndex = -1
				option.style.cssText += `;display:block;width:100%;text-align:left;background:${i === index ? "#dbeafe" : "white"}`
				if (i === index) search.setAttribute("aria-activedescendant", option.id)
			})
			if (!matches.length) list.textContent = "No matching tools"
		}
		search.oninput = () => {
			const words = search.value.toLowerCase().trim().split(/\s+/)
			matches = actions.filter((action) => words.every((word) => action.textContent?.toLowerCase().includes(word)))
			index = 0
			render()
		}
		panel.onkeydown = (event) => {
			event.stopPropagation()
			if (event.key === "Escape") {
				event.preventDefault()
				this.closeToolPalette()
				this.svg.focus()
			} else if (event.key === "Enter") {
				event.preventDefault()
				activate(matches[index])
			} else if (["ArrowDown", "ArrowUp"].includes(event.key)) {
				event.preventDefault()
				index = matches.length ? (index + (event.key === "ArrowDown" ? 1 : matches.length - 1)) % matches.length : 0
				render()
				list.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" })
			} else if (event.key === "Tab") this.closeToolPalette()
		}
		panel.append(search, list)
		this.root.append(panel)
		render()
		search.focus()
	}
	private compactTools() {
		const persistent = new Set(["Search tools", "Select", "Undo", "Redo", "Fit sketch", "Finish sketch", "Cancel sketch", "Finish spline", "Close spline"])
		const controls = Array.from(this.tools.children)
		const toolGroup = (name: string) => (name === "Dimension" ? "Constraints" : ["Trim", "Split", "Extend"].includes(name) ? "Edit" : "Draw")
		for (const category of ["Draw", "Edit", "Constraints", "Options"]) {
			const menu = document.createElement("details")
			this.tools.style.position = "relative"
			const summary = document.createElement("summary")
			summary.textContent = category === toolGroup(this.tool) && this.tool !== "Select" ? `${category} · ${this.tool}` : category
			summary.style.cssText = "cursor:pointer;padding:6px;border:1px solid #ccd4df;border-radius:4px;background:white"
			const panel = document.createElement("div")
			panel.style.cssText =
				"position:absolute;top:100%;left:0;z-index:6;width:min(420px,90vw);max-height:50vh;overflow:auto;display:flex;flex-wrap:wrap;gap:5px;padding:8px;background:white;border:1px solid #ccd4df;box-shadow:0 5px 15px #17203333"
			panel.setAttribute("aria-label", `${category} tools`)
			for (const control of controls) {
				if (persistent.has(control.textContent ?? "")) continue
				const group =
					control.tagName === "LABEL"
						? "Options"
						: control.hasAttribute("aria-pressed")
							? toolGroup(control.textContent ?? "")
							: control.hasAttribute("data-constraint-tool")
								? "Constraints"
								: "Edit"
				if (group === category) panel.append(control)
			}
			menu.append(summary, panel)
			summary.onclick = () => {
				for (const other of Array.from(this.tools.querySelectorAll("details"))) if (other !== menu) other.open = false
			}
			panel.addEventListener("click", (event) => {
				if (event.target instanceof window.HTMLButtonElement) {
					menu.open = false
					if (!this.disposed) this.svg.focus()
				}
			})
			menu.onkeydown = (event) => {
				if (event.key === "Escape") {
					menu.open = false
					summary.focus()
					event.stopPropagation()
				}
			}
			this.tools.insertBefore(menu, this.tools.querySelector(":scope > button"))
		}
	}
	private renderTools() {
		this.closeToolPalette()
		this.tools.replaceChildren()
		this.button(this.tools, "Search tools", () => this.openToolPalette()).title = "Search sketch tools (S)"
		for (const name of [
			"Select",
			"Fit spline",
			"Control spline",
			"Ellipse",
			"Polygon",
			"Point",
			"Line",
			"Rectangle",
			"Center rectangle",
			"3-point rectangle",
			"3-point center rectangle",
			"Circle",
			"3-point circle",
			"Diameter circle",
			"3-point arc",
			"Center arc",
			"Tangent arc",
			"Slot",
			"Dimension",
			"Trim",
			"Split",
			"Extend"
		] as Tool[]) {
			const b = this.button(this.tools, name, () => {
				this.cancelDrag()
				this.tool = name
				this.splinePoints = []
				this.hoverSnap = null
				this.tangentStart = null
				this.start = null
				this.secondPoint = null
				this.splitCircleStart = null
				this.renderProperties()
				this.renderTools()
				this.draw()
			})
			b.setAttribute("aria-pressed", String(this.tool === name))
			if (name === "Select") b.title = "Drag left-to-right to contain, right-to-left to cross. Shift adds to selection."
			if (this.tool === name) b.style.background = "#dbeafe"
		}
		if (this.tool === "Fit spline" || this.tool === "Control spline") {
			const finish = this.button(this.tools, "Finish spline", () => this.finishSpline())
			finish.disabled = this.tool === "Fit spline" ? this.splinePoints.length < 2 : this.splinePoints.length < 4 || (this.splinePoints.length - 1) % 3 !== 0
		}
		if (this.tool === "Fit spline") this.button(this.tools, "Close spline", () => this.finishSpline(true)).disabled = this.splinePoints.length < 3
		if (this.tool === "Center arc")
			this.checkbox(this.tools, "Clockwise arc", this.clockwiseArc, (value) => {
				this.clockwiseArc = value
				this.draw()
			})
		this.button(this.tools, "Sketch chamfer", () => {
			let removed: string[] = []
			this.edit(() => {
				if (this.selected.length !== 2) throw Error("Select two connected lines for a sketch chamfer.")
				const result = chamferSketchCorner(
					this.sketch,
					requireValue(this.selected[0]),
					requireValue(this.selected[1]),
					this.chamferFirst,
					this.chamferAngleMode ? this.chamferAngle : this.chamferSecond,
					this.chamferAngleMode ? "distance-angle" : "two-distances"
				)
				this.sketch = result.sketch
				this.selected = [result.lineId]
				this.selectedRelation = result.dimensionId
				this.tool = "Select"
				removed = result.removedRelations
			})
			if (removed.length) this.status.textContent += ` · Removed corner constraints: ${removed.join(", ")}`
		})
		this.button(this.tools, "Sketch fillet", () => {
			let removed: string[] = []
			this.edit(() => {
				if (this.selected.length !== 2) throw Error("Select two connected lines for a sketch fillet.")
				const result = filletSketchCorner(this.sketch, requireValue(this.selected[0]), requireValue(this.selected[1]), this.filletRadius)
				this.sketch = result.sketch
				this.selected = [result.arcId]
				this.selectedRelation = result.radiusId
				this.tool = "Select"
				this.start = null
				this.secondPoint = null
				this.anchors = []
				removed = result.removedRelations
			})
			if (removed.length) this.status.textContent += ` · Removed constraints: ${removed.join(", ")}`
		})
		this.button(this.tools, "Linear pattern", () =>
			this.edit(() => {
				const result = createLinearSketchPattern(this.sketch, this.selected, this.selected.length > 32 ? 2 : 3, { x: 20, y: 0 })
				this.sketch = result.sketch
				this.selectedRelation = result.patternId
				this.selected = []
				this.anchors = []
				this.tool = "Select"
				this.start = null
				this.secondPoint = null
			})
		)
		this.button(this.tools, "Circular pattern", () =>
			this.edit(() => {
				const result = createCircularSketchPattern(this.sketch, this.selected, this.selected.length > 21 ? 2 : 4, { x: 0, y: 0 })
				this.sketch = result.sketch
				this.selectedRelation = result.patternId
				this.selected = []
				this.anchors = []
				this.tool = "Select"
				this.start = null
				this.secondPoint = null
			})
		)
		this.button(this.tools, "Mirror", () =>
			this.edit(() => {
				const axis = need(
					this.sketch.entities.find((e) => e.id === this.selected[this.selected.length - 1]),
					"Select geometry, then Shift-select a mirror line last."
				)
				if (axis.type !== "line" || this.selected.length < 2) throw Error("Select geometry, then Shift-select a mirror line last.")
				const sources = this.sketch.entities.filter((e) => this.selected.includes(e.id) && e.id !== axis.id)
				const copies: string[] = []
				for (const source of sources) {
					const copy = mirrorSketchEntity(source, axis, this.id("mirror"))
					this.sketch.entities.push(copy)
					const relation: SketchRelation = { id: this.id("mirror-link"), type: "mirror", a: source.id, b: copy.id, symmetryLine: axis.id }
					this.sketch.relations?.push(relation)
					copies.push(copy.id)
				}
				this.selected = copies
				this.selectedRelation = null
				this.tool = "Select"
				this.start = null
				this.secondPoint = null
				this.anchors = []
			})
		)
		this.button(this.tools, "Offset", () =>
			this.edit(() => {
				const source = need(
					this.sketch.entities.find((e) => e.id === this.selected[0]),
					"Select one entity to offset."
				)
				if (this.selected.length > 1) {
					const sources = this.sketch.entities.filter((e) => this.selected.includes(e.id))
					const definition = orderOffsetChain(sources)
					const targets: string[] = []
					for (const item of definition.sources) {
						const sourceEntity = need(
							sources.find((e) => e.id === item.entityId),
							"Missing offset source."
						)
						const target = offsetSketchEntity(sourceEntity, 0, this.id("offset"))
						targets.push(target.id)
						this.sketch.entities.push(target)
					}
					const geometry = offsetLineChain(sources, definition, 5, targets)
					this.sketch.entities = this.sketch.entities.map((e) => geometry.find((line) => line.id === e.id) ?? e)
					const relation: SketchRelation = { id: this.id("offset-chain"), type: "offsetChain", ...definition, targets, value: 5 }
					this.sketch.relations?.push(relation)
					this.selected = targets
					this.selectedRelation = relation.id
					return
				}
				const target = offsetSketchEntity(source, 5, this.id("offset"))
				this.sketch.entities.push(target)
				const relation: SketchRelation = { id: this.id("offset-distance"), type: "offset", a: source.id, b: target.id, value: 5 }
				this.sketch.relations?.push(relation)
				this.selected = [target.id]
				this.selectedRelation = relation.id
			})
		)
		this.checkbox(this.tools, "Construction", this.construction, (value) => {
			this.construction = value
			if (this.selected.length)
				this.edit(() => {
					for (const e of this.sketch.entities) if (this.selected.includes(e.id)) e.construction = value
				})
		})
		if (this.supportEdges.length)
			this.checkbox(this.tools, "Show support", this.showSupport, (value) => {
				this.showSupport = value
				this.draw()
			})
		this.checkbox(this.tools, "Snap to grid", this.snapGrid, (value) => {
			this.snapGrid = value
			this.hoverSnap = null
			this.draw()
		})
		this.checkbox(this.tools, "Automatic constraints", this.autoConstraints, (value) => {
			this.autoConstraints = value
		})
		this.checkbox(this.tools, "Snap to geometry", this.snapGeometry, (value) => {
			this.snapGeometry = value
			this.hoverSnap = null
			this.draw()
		})
		this.checkbox(this.tools, "Show constraints", this.showConstraints, (value) => {
			this.showConstraints = value
			this.draw()
		})
		for (const name of [
			"Horizontal",
			"Vertical",
			"Coincident",
			"Symmetric",
			"Symmetric entities",
			"Midpoint",
			"Point on curve",
			"Normal",
			"Concentric",
			"Equal",
			"Parallel",
			"Collinear",
			"Perpendicular",
			"Tangent",
			"Internal tangent",
			"Fix",
			"Unfix",
			"Distance",
			"X distance",
			"Y distance",
			"Radial distance",
			"Angle"
		])
			this.button(this.tools, name, () => this.constrain(name)).setAttribute("data-constraint-tool", "")
		this.button(this.tools, "Undo", () => this.history(false)).disabled = !this.undo.length
		this.button(this.tools, "Redo", () => this.history(true)).disabled = !this.redo.length
		this.button(this.tools, "Fit sketch", () => this.fit())
		this.button(this.tools, "Finish sketch", () => this.finishSketch()).style.background = "#dcfce7"
		this.button(this.tools, "Cancel sketch", () => {
			if (this.disposed) return
			this.dispose()
			this.cancel()
		})
		this.compactTools()
	}
	private checkbox(parent: HTMLElement, label: string, checked: boolean, change: (value: boolean) => void) {
		const l = document.createElement("label")
		const input = document.createElement("input")
		input.type = "checkbox"
		input.checked = checked
		input.onchange = () => change(input.checked)
		l.append(input, document.createTextNode(label))
		parent.append(l)
		return input
	}
	private variablesOpen = false
	private parseValue(text: string, kind: SketchValueKind) {
		const values = evaluateSketchVariables(this.sketch.variables ?? [])
		return parseSketchValue(text, kind, (name) => {
			const value = values.get(name)
			if (!value) throw Error(`Unknown sketch variable: #${name}`)
			return value
		})
	}
	private renderVariables() {
		const details = document.createElement("details")
		details.open = this.variablesOpen
		const summary = document.createElement("summary")
		summary.textContent = "Variables"
		details.ontoggle = () => {
			this.variablesOpen = details.open
		}
		details.append(summary)
		const update = (variables: SketchVariable[]) =>
			this.edit(() => {
				this.sketch = materializeSketch({ ...this.sketch, variables })
			})
		const variables = this.sketch.variables ?? []
		for (const variable of variables) {
			const row = document.createElement("div")
			row.style.cssText = "border-bottom:1px solid #ccd4df;padding:6px 0"
			const replace = (change: Partial<SketchVariable>) => {
				if (change.name !== undefined)
					this.edit(() => {
						this.sketch = materializeSketch(renameSketchVariable(this.sketch, variable.name, change.name ?? variable.name))
					})
				else update(variables.map((item) => (item.name === variable.name ? { ...item, ...change } : item)))
			}
			for (const key of ["name", "expression"] as const) {
				const label = document.createElement("label")
				label.textContent = key === "name" ? "Name" : "Expression"
				const input = document.createElement("input")
				input.value = variable[key]
				input.setAttribute("aria-label", `Variable ${variable.name} ${key}`)
				input.style.cssText = "display:block;width:100%;box-sizing:border-box"
				input.onchange = () => replace({ [key]: input.value.trim() })
				label.append(input)
				row.append(label)
			}
			const kind = document.createElement("select")
			kind.setAttribute("aria-label", `Variable ${variable.name} type`)
			for (const value of ["length", "angle", "scalar"] as const) {
				const option = document.createElement("option")
				option.value = value
				option.textContent = value === "scalar" ? "Number" : value === "length" ? "Length (mm)" : "Angle (degrees)"
				kind.append(option)
			}
			kind.value = variable.kind
			kind.onchange = () => replace({ kind: kind.value as SketchValueKind })
			row.append(kind)
			this.button(row, `Delete #${variable.name}`, () => update(variables.filter((item) => item.name !== variable.name)))
			details.append(row)
		}
		this.button(details, "Add variable", () => {
			let index = 1
			while (variables.some((item) => item.name === `variable${index}`)) index++
			update([...variables, { name: `variable${index}`, kind: "length", expression: "10 mm" }])
		})
		this.properties.append(details)
	}
	private field(label: string, value: number | string, change: (value: number, expression: string) => void, kind: SketchValueKind = "scalar") {
		const l = document.createElement("label")
		const input = document.createElement("input")
		l.style.cssText = "display:block;margin:10px 0"
		input.type = "text"
		input.value = String(value)
		input.setAttribute("aria-label", label)
		input.style.cssText = "display:block;width:100%;box-sizing:border-box;padding:6px"
		input.onchange = () => {
			try {
				const v = this.parseValue(input.value, kind)
				input.removeAttribute("aria-invalid")
				change(v, input.value.trim())
			} catch (error) {
				input.setAttribute("aria-invalid", "true")
				this.status.textContent = error instanceof Error ? error.message : String(error)
			}
		}
		l.append(document.createTextNode(label), input)
		this.properties.append(l)
	}
	private renderProperties() {
		this.properties.replaceChildren()
		const heading = document.createElement("h2")
		heading.textContent = this.sketch.name ?? "Sketch"
		heading.style.margin = "0 0 12px"
		this.properties.append(heading)
		this.renderVariables()
		const filterLabel = document.createElement("label")
		filterLabel.textContent = "Selection filter"
		const filter = document.createElement("select")
		filter.setAttribute("aria-label", "Selection filter")
		filter.style.cssText = "display:block;width:100%;padding:6px"
		for (const name of selectionFilters) {
			const option = document.createElement("option")
			option.value = name
			option.textContent = name
			option.selected = name === this.selectionFilter
			filter.append(option)
		}
		filter.onchange = () => {
			const value = selectionFilters.find((name) => name === filter.value)
			if (!value) return
			this.cancelDrag()
			this.selectionFilter = value
			this.selected = this.selected.filter((id) => this.sketch.entities.some((e) => e.id === id && this.canSelect(e)))
			this.anchors = this.anchors.filter((a) => this.selected.includes(a.entityId))
			this.renderProperties()
			this.draw()
		}
		filterLabel.append(filter)
		this.properties.append(filterLabel)

		const plane = document.createElement("p")
		plane.textContent = this.sketch.target.type === "plane" ? `${this.sketch.target.plane} plane` : "Attached face"
		this.properties.append(plane)
		if (this.supportError) {
			const error = document.createElement("p")
			error.setAttribute("role", "alert")
			error.setAttribute("data-support-error", "")
			error.style.cssText = "color:#b91c1c;padding:8px;border:1px solid #fecaca;background:#fff1f2"
			error.textContent = `Sketch support unavailable: ${this.supportError} Close the sketch and choose a valid Sketch support to reattach it.`
			this.properties.append(error)
		}
		const help = document.createElement("p")
		help.setAttribute("aria-label", "Sketch tool instructions")
		const splineHelp: Partial<Record<Tool, string>> = {
			"Fit spline":
				"Pick points for the curve to pass through. Enter or Finish spline creates an open curve. With at least three points, Close spline creates a smooth loop. Escape cancels placement.",
			"Control spline":
				"Pick a start point, two control handles, then an end point. Repeat two handles and an end for more segments. Enter or Finish spline completes the curve. Escape cancels placement.",
			Trim: "Click the curve section to remove between crossings. Use lines, circles, arcs, ellipses, elliptic arcs, splines, rectangles, polygons or slots as boundaries. Retained fit-spline pieces become editable control splines. Without crossings, Trim removes the whole curve.",
			Split: "Click a curve to split it. Splitting a closed fit spline opens it at the clicked point while preserving its shape. Fit splines become control splines. For a circle or ellipse, pick two split points. Elliptical pieces remain on a shared ellipse.",
			Extend: "Click near the endpoint of a line, circular arc or elliptic arc to extend it to the next boundary. Closed circles and ellipses have no endpoints. Spline extension is not available."
		}
		help.textContent =
			this.tool === "Diameter circle"
				? "Pick two opposite points on the circle to set its diameter. Escape cancels placement."
				: this.tool === "3-point center rectangle"
					? "Pick the center, then the midpoint of one side to set width and rotation. Pick an adjacent side to set height. Escape cancels placement."
					: "Slot: two cap centers, then width. 3-point circle: three points on its circumference. 3-point arc: start, end, then a point on the curve. Center arc: center, start, then end direction. Tangent arc: existing line, arc, elliptic arc or spline endpoint, then end point. Shift-click selects several entities or points. Drag endpoints to reshape; middle-drag pans. D adds a dimension; Escape cancels a tool."
		help.textContent = splineHelp[this.tool] ?? help.textContent
		this.properties.append(help)
		const relation = this.sketch.relations?.find((r) => r.id === this.selectedRelation)
		if (relation) {
			if (this.result.redundantRelations.includes(relation.id)) {
				const note = document.createElement("p")
				note.textContent = "Locally redundant with earlier constraints. Review the geometry before deleting this relation."
				this.properties.append(note)
			}
			const h = document.createElement("h3")
			h.textContent = `${constraintLabel(relation.type)} dimension / constraint`
			this.properties.append(h)
			if (relation.type === "linearPattern") {
				const { columns, rows, rowStep } = patternLayout(relation)
				const update = (count: number, step: Point2D, newRows = rows, newRowStep = rowStep) => {
					let removed: string[] = []
					this.edit(() => {
						const result = resizeLinearSketchPattern(this.sketch, relation.id, count, step, newRows, newRowStep)
						this.sketch = result.sketch
						this.selected = this.selected.filter((id) => this.sketch.entities.some((e) => e.id === id))
						removed = result.removedRelations
					})
					if (removed.length) this.status.textContent += ` · Removed constraints: ${removed.join(", ")}`
				}
				this.field("Pattern columns", columns, (count) => update(count, relation.step))
				this.field("Pattern rows", rows, (value) => update(columns, relation.step, value))
				this.field("Row step X (mm)", rowStep.x, (x) => update(columns, relation.step, rows, { ...rowStep, x }), "length")
				this.field("Row step Y (mm)", rowStep.y, (y) => update(columns, relation.step, rows, { ...rowStep, y }), "length")
				this.field("Pattern step X (mm)", relation.step.x, (x) => update(columns, { ...relation.step, x }), "length")
				this.field("Pattern step Y (mm)", relation.step.y, (y) => update(columns, { ...relation.step, y }), "length")
			}
			if (relation.type === "circularPattern") {
				const count = relation.instances.length + 1
				const update = (newCount: number, center: Point2D, angle: number) => {
					let removed: string[] = []
					this.edit(() => {
						const result = resizeCircularSketchPattern(this.sketch, relation.id, newCount, center, angle)
						this.sketch = result.sketch
						this.selected = this.selected.filter((id) => this.sketch.entities.some((e) => e.id === id))
						removed = result.removedRelations
					})
					if (removed.length) this.status.textContent += ` · Removed constraints: ${removed.join(", ")}`
				}
				this.field("Pattern count", count, (value) => update(value, relation.center, relation.angle))
				const center = circularPatternCenter(relation, this.sketch.entities)
				const label = document.createElement("label")
				label.textContent = "Pattern center reference"
				label.style.cssText = "display:block;margin:10px 0"
				const reference = document.createElement("select")
				reference.setAttribute("aria-label", "Pattern center reference")
				reference.style.cssText = "display:block;width:100%;box-sizing:border-box;padding:6px"
				const free = document.createElement("option")
				free.value = ""
				free.textContent = "Free center"
				reference.append(free)
				const choices: SketchAnchor[] = []
				const copies = new Set(relation.instances.flat())
				for (const entity of this.sketch.entities)
					if (!copies.has(entity.id))
						for (const point of entityAnchorNames(entity)) {
							const option = document.createElement("option")
							option.value = String(choices.length)
							option.textContent = `${entity.id} · ${point}`
							option.selected = relation.centerAnchor?.entityId === entity.id && relation.centerAnchor.point === point
							choices.push({ entityId: entity.id, point })
							reference.append(option)
						}
				reference.onchange = () =>
					this.edit(() => {
						this.sketch = setCircularPatternCenter(this.sketch, relation.id, reference.value === "" ? null : requireValue(choices[Number(reference.value)])).sketch
					})
				label.append(reference)
				this.properties.append(label)
				if (!relation.centerAnchor) {
					this.field("Pattern center X (mm)", center.x, (x) => update(count, { ...center, x }, relation.angle), "length")
					this.field("Pattern center Y (mm)", center.y, (y) => update(count, { ...center, y }, relation.angle), "length")
				}
				this.field("Pattern angle (degrees)", relation.angle, (angle) => update(count, relation.center, angle), "angle")
			}
			if (isMeasuredDimension(relation))
				this.checkbox(this.properties, "Reference dimension", relation.reference === true, (value) =>
					this.edit(() => {
						if ("value" in relation) relation.value = measureSketchDimension(this.sketch.entities, relation)
						relation.expression = undefined
						relation.reference = value
					})
				)
			if (relation.reference) {
				const output = document.createElement("output")
				output.setAttribute("aria-label", "Reference dimension value")
				output.textContent = `(${Number(measureSketchDimension(this.sketch.entities, relation).toFixed(3))})`
				this.properties.append(output)
			}
			if (relation.type === "chamfer") {
				this.checkbox(this.properties, "Chamfer distance and angle", relation.mode === "distance-angle", (value) =>
					this.edit(() => {
						this.sketch = setChamferMode(this.sketch, relation.id, value ? "distance-angle" : "two-distances")
					})
				)
				this.field(
					relation.mode === "distance-angle" ? "Chamfer angle (degrees)" : "Second setback (mm)",
					relation.secondValue,
					(value) =>
						this.edit(() => {
							relation.secondValue = value
						}),
					relation.mode === "distance-angle" ? "angle" : "length"
				)
			}
			if ("value" in relation && !relation.reference)
				this.field(
					relation.type === "angle" || relation.type === "arcSweep" || relation.type === "rotation" ? "Angle (degrees)" : "Dimension (mm)",
					relation.expression ?? relation.value,
					(v, expression) =>
						this.edit(() => {
							relation.value = v
							if (isMeasuredDimension(relation)) {
								if (expression === String(v)) relation.expression = undefined
								else relation.expression = expression
							}
						}),
					["angle", "arcSweep", "rotation"].includes(relation.type) ? "angle" : "length"
				)
			this.button(this.properties, "Delete constraint", () =>
				this.edit(() => {
					this.sketch.relations = this.sketch.relations?.filter((r) => r.id !== relation.id)
					this.selectedRelation = null
				})
			)
		}
		if (!relation && this.selected.length === 2 && this.selected.every((id) => this.sketch.entities.find((e) => e.id === id)?.type === "line"))
			this.field(
				"Fillet radius (mm)",
				this.filletRadius,
				(value) => {
					this.filletRadius = value
				},
				"length"
			)
		if (!relation && this.selected.length === 2 && this.selected.every((id) => this.sketch.entities.find((e) => e.id === id)?.type === "line")) {
			this.field(
				"First chamfer setback (mm)",
				this.chamferFirst,
				(value) => {
					this.chamferFirst = value
				},
				"length"
			)
			this.checkbox(this.properties, "Chamfer distance and angle", this.chamferAngleMode, (value) => {
				this.chamferAngleMode = value
				this.renderProperties()
			})
			this.field(
				this.chamferAngleMode ? "Chamfer angle (degrees)" : "Second chamfer setback (mm)",
				this.chamferAngleMode ? this.chamferAngle : this.chamferSecond,
				(value) => {
					if (this.chamferAngleMode) this.chamferAngle = value
					else this.chamferSecond = value
				},
				this.chamferAngleMode ? "angle" : "length"
			)
		}
		if (this.tool === "Polygon") {
			this.field("Polygon sides", this.polygonSides, (value) => {
				if (!Number.isInteger(value) || value < 3 || value > 64) {
					this.status.textContent = "Polygon sides must be an integer from 3 to 64."
					return
				}
				this.polygonSides = value
				this.draw()
			})
			this.checkbox(this.properties, "Circumscribed polygon", this.circumscribedPolygon, (value) => {
				this.circumscribedPolygon = value
				this.draw()
			})
		}
		const entity = this.sketch.entities.find((e) => e.id === this.selected[0])
		if (entity && !relation) {
			if (entity.type === "ellipticArc")
				for (const key of ["startAngle", "sweep"] as const)
					this.field(
						key === "startAngle" ? "Arc start (degrees)" : "Arc sweep (degrees)",
						(entity[key] * 180) / Math.PI,
						(value) =>
							this.edit(() => {
								entity[key] = (value * Math.PI) / 180
								normalizeEllipticArc(entity)
							}),
						"angle"
					)
			if (entity.type === "spline" && entity.mode === "fit")
				this.checkbox(this.properties, "Closed spline", entity.closed === true, (closed) =>
					this.edit(() => {
						entity.closed = closed
						normalizeSpline(entity)
					})
				)
			if (entity.type === "spline")
				entity.points.forEach((point, i) => {
					for (const axis of ["x", "y"] as const)
						this.field(`Spline point ${i + 1} ${axis.toUpperCase()} (mm)`, point[axis], (value) =>
							this.edit(() => {
								point[axis] = value
								normalizeSpline(entity)
							})
						)
				})
			if (entity.type === "polygon")
				this.field("Polygon side count", entity.sides, (value) => {
					let removed: string[] = []
					this.edit(() => {
						const result = setPolygonSideCount(this.sketch, entity.id, value)
						this.sketch = result.sketch
						this.anchors = this.anchors.filter((anchor) => {
							const owner = this.sketch.entities.find((entity) => entity.id === anchor.entityId)
							return owner && entityAnchorNames(owner).includes(anchor.point)
						})
						removed = result.removedRelations
					})
					if (removed.length) this.status.textContent += ` · Removed constraints: ${removed.join(", ")}`
				})
			const h = document.createElement("h3")
			h.textContent = `${entity.type} · ${entity.id}`
			this.properties.append(h)
			const selectedEntities = this.sketch.entities.filter((e) => this.selected.includes(e.id))
			const constructionCount = selectedEntities.filter((e) => e.construction).length
			const constructionControl = this.checkbox(this.properties, "Construction geometry", constructionCount === selectedEntities.length, (value) =>
				this.edit(() => {
					for (const selectedEntity of selectedEntities) selectedEntity.construction = value
				})
			)
			constructionControl.indeterminate = constructionCount > 0 && constructionCount < selectedEntities.length
			if (entity.type !== "point" && entity.type !== "spline") this.button(this.properties, "Add driving dimension", () => this.addDimension(entity))
			if (entity.type !== "point" && entity.type !== "spline") this.button(this.properties, "Add reference dimension", () => this.addDimension(entity, undefined, true))
			if (entity.type === "capsule")
				this.button(this.properties, "Dimension slot center distance", () =>
					this.addRelation({
						id: this.id("dimension"),
						type: "distance",
						a: { entityId: entity.id, point: "from" },
						b: { entityId: entity.id, point: "to" },
						value: Math.hypot(entity.to.x - entity.from.x, entity.to.y - entity.from.y)
					})
				)
			if (entity.type === "arc" || entity.type === "ellipticArc")
				this.button(this.properties, "Dimension arc sweep", () =>
					this.addRelation({ id: this.id("dimension"), type: "arcSweep", entityId: entity.id, value: (entity.sweep * 180) / Math.PI })
				)
			if (entity.type === "ellipticArc" || entity.type === "ellipse" || entity.type === "rectangle" || entity.type === "polygon")
				this.button(this.properties, "Dimension rotation", () => this.addRelation({ id: this.id("dimension"), type: "rotation", entityId: entity.id, value: entity.rotation }))
			if (entity.type === "ellipticArc" || entity.type === "ellipse" || entity.type === "rectangle" || entity.type === "cornerRectangle") {
				this.button(this.properties, "Dimension width", () => this.addDimension(entity, "width"))
				this.button(this.properties, "Dimension height", () => this.addDimension(entity, "height"))
			}
			this.button(this.properties, "Delete selected", () => this.deleteSelected())
		}
		if (this.selected.length && !relation) {
			const heading = document.createElement("h3")
			heading.textContent = "Move / copy"
			this.properties.append(heading)
			this.field(
				"Move X (mm)",
				this.moveX,
				(value) => {
					this.moveX = value
				},
				"length"
			)
			this.field(
				"Move Y (mm)",
				this.moveY,
				(value) => {
					this.moveY = value
				},
				"length"
			)
			for (const copy of [false, true]) this.button(this.properties, copy ? "Copy selected" : "Move selected", () => this.transformSelection(copy, "translate"))
			const scaleHeading = document.createElement("h3")
			scaleHeading.textContent = "Scale"
			this.properties.append(scaleHeading)
			this.field("Scale factor", this.scaleFactor, (value) => {
				this.scaleFactor = value
			})
			this.field(
				"Scale center X (mm)",
				this.scaleCenter.x,
				(value) => {
					this.scaleCenter.x = value
					this.draw()
				},
				"length"
			)
			this.field(
				"Scale center Y (mm)",
				this.scaleCenter.y,
				(value) => {
					this.scaleCenter.y = value
					this.draw()
				},
				"length"
			)
			for (const copy of [false, true]) this.button(this.properties, copy ? "Scale copy" : "Scale selected", () => this.transformSelection(copy, "scale"))
			const rotationHeading = document.createElement("h3")
			rotationHeading.textContent = "Rotate"
			this.properties.append(rotationHeading)
			this.field(
				"Rotate angle (degrees)",
				this.rotationAngle,
				(value) => {
					this.rotationAngle = value
				},
				"angle"
			)
			this.field(
				"Rotate center X (mm)",
				this.rotationCenter.x,
				(value) => {
					this.rotationCenter.x = value
					this.draw()
				},
				"length"
			)
			this.field(
				"Rotate center Y (mm)",
				this.rotationCenter.y,
				(value) => {
					this.rotationCenter.y = value
					this.draw()
				},
				"length"
			)
			for (const copy of [false, true]) this.button(this.properties, copy ? "Rotate copy" : "Rotate selected", () => this.transformSelection(copy, "rotate"))
		}
		const entities = document.createElement("details")
		const summary = document.createElement("summary")
		summary.textContent = `Entities (${this.sketch.entities.length})`
		entities.append(summary)
		entities.open = true
		this.properties.append(entities)
		for (const e of this.sketch.entities) {
			if (!this.canSelect(e)) continue
			const b = this.button(entities, `Select ${e.id}`, () => undefined)
			b.onclick = (event) => this.select(e.id, event.shiftKey)
			b.setAttribute("aria-pressed", String(this.selected.includes(e.id)))
			const state = this.result.entityStates[e.id] ?? "unknown"
			const caption = document.createElement("span")
			caption.textContent =
				state === "fully-constrained" ? " Fully constrained" : state === "underconstrained" ? " Can move" : state === "conflicting" ? " Conflicting" : " State uncertain"
			caption.style.cssText = "font-size:12px;margin-right:8px"
			caption.setAttribute("data-entity-constraint-state", state)
			caption.setAttribute("data-state-entity", e.id)
			entities.append(caption)
		}

		const h = document.createElement("h3")
		h.textContent = "Constraints"
		this.properties.append(h)
		for (const r of this.sketch.relations ?? []) {
			const b = this.button(
				this.properties,
				`${constraintLabel(r.type)}${"value" in r ? ` ${Number((r.reference ? measureSketchDimension(this.sketch.entities, r) : r.value).toFixed(3))}` : ""} · ${r.id}`,
				() => {
					this.selectedRelation = r.id
					this.renderProperties()
					this.draw()
				}
			)
			b.style.cssText += ";display:block;width:100%;text-align:left;margin:4px 0"
			if (this.result.conflicts.includes(r.id)) b.style.color = "#b91c1c"
			else if (this.result.redundantRelations.includes(r.id)) {
				b.style.color = "#a16207"
				b.textContent += " · redundant"
				b.setAttribute("data-redundant-relation", r.id)
				b.title = "Adds no local restriction beyond earlier constraints. Review before deleting."
			}
		}
	}
	private transformSelection(copy: boolean, mode: "translate" | "scale" | "rotate") {
		let removed: string[] = []
		this.edit(() => {
			const result =
				mode === "rotate"
					? rotateSketchSelection(this.sketch, this.selected, this.rotationCenter, this.rotationAngle, copy)
					: mode === "scale"
						? scaleSketchSelection(this.sketch, this.selected, this.scaleCenter, this.scaleFactor, copy)
						: translateSketchSelection(this.sketch, this.selected, { x: this.moveX, y: this.moveY }, copy)
			this.sketch = result.sketch
			this.selected = result.selected
			this.selectedRelation = null
			this.anchors = []
			this.tool = "Select"
			removed = result.removedRelations
		})
		if (removed.length) this.status.textContent += ` · Detached external constraints: ${removed.join(", ")}`
	}
	private edit(action: () => void) {
		if (this.disposed) return
		const before = copy(this.sketch)
		try {
			action()
			this.result = solveSketch(this.sketch.entities, this.sketch.relations ?? [])
			if (this.result.status !== "conflicting") this.sketch.entities = this.result.entities
			this.selected = this.selected.filter((id) => this.sketch.entities.some((e) => e.id === id && this.canSelect(e)))
			this.anchors = this.anchors.filter((a) => this.selected.includes(a.entityId))
			this.undo.push(before)
			this.redo = []
			this.renderTools()
			this.renderProperties()
			this.draw()
		} catch (error) {
			this.sketch = before
			this.result = solveSketch(before.entities, before.relations ?? [])
			this.renderTools()
			this.renderProperties()
			this.draw()
			this.status.textContent = error instanceof Error ? error.message : String(error)
		}
	}
	private id(prefix: string) {
		let n = 1
		const ids = new Set([...this.sketch.entities, ...(this.sketch.relations ?? [])].map((e) => e.id))
		while (ids.has(`${prefix}-${n}`)) n++
		return `${prefix}-${n}`
	}
	private addRelation(relation: SketchRelation) {
		this.edit(() => {
			this.sketch.relations?.push(relation)
			this.selectedRelation = relation.id
		})
	}
	private defaultAnchor(e: SketchEntity): SketchAnchor {
		return {
			entityId: e.id,
			point:
				e.type === "spline"
					? "point0"
					: e.type === "ellipse" ||
							e.type === "ellipticArc" ||
							e.type === "polygon" ||
							e.type === "point" ||
							e.type === "circle" ||
							e.type === "arc" ||
							e.type === "rectangle"
						? "center"
						: e.type === "capsule"
							? "from"
							: "p0"
		}
	}
	private anchorPoint(a: SketchAnchor): Point2D {
		const e = requireValue(this.sketch.entities.find((e) => e.id === a.entityId))
		return entityAnchorPoint(e, a.point)
	}
	private constrain(name: string) {
		try {
			const a = need(
				this.sketch.entities.find((e) => e.id === this.selected[0]),
				"Select an entity first."
			)
			const b = this.sketch.entities.find((e) => e.id === this.selected[1])
			const id = this.id("constraint")
			if (name === "Horizontal" || name === "Vertical") {
				const group = this.selected.map((selectedId) => requireValue(this.sketch.entities.find((entity) => entity.id === selectedId)))
				const points =
					this.anchors.length >= 2
						? this.anchors
						: group.length >= 2 && group.every((entity) => entity.type === "point")
							? group.map((entity) => this.defaultAnchor(entity))
							: null
				if (points)
					return this.edit(() => {
						for (const point of points.slice(1)) {
							const relation: SketchRelation = {
								id: this.id("constraint"),
								type: "distance",
								a: requireValue(points[0]),
								b: point,
								axis: name === "Horizontal" ? "y" : "x",
								value: 0
							}
							this.sketch.relations?.push(relation)
							this.selectedRelation = relation.id
						}
					})
				if (group.some((entity) => entity.type !== "line")) throw Error("Select only lines for Horizontal or Vertical.")
				const type = name === "Horizontal" ? "horizontal" : "vertical"
				const missing = group.filter((entity) => !this.sketch.relations?.some((relation) => relation.type === type && relation.entityId === entity.id && !relation.reference))
				if (!missing.length) {
					this.status.textContent = `All selected lines already have ${name} constraints.`
					return
				}
				return this.edit(() => {
					for (const entity of missing) {
						const relation: SketchRelation = { id: this.id("constraint"), type, entityId: entity.id }
						this.sketch.relations?.push(relation)
						this.selectedRelation = relation.id
					}
				})
			}
			const anchor = this.anchors[0] ?? this.defaultAnchor(a)
			if (name === "Unfix") {
				const removable = (r: SketchRelation) =>
					this.anchors.length
						? r.type === "fixed" && this.anchors.some((point) => point.entityId === r.anchor.entityId && point.point === r.anchor.point)
						: !!r.fixation && this.selected.includes(r.fixation)
				if (!this.sketch.relations?.some(removable)) {
					this.status.textContent = "No Fix constraints found for this selection."
					return
				}
				return this.edit(() => {
					this.sketch.relations = this.sketch.relations?.filter((r) => !removable(r))
					this.selectedRelation = null
				})
			}
			if (name === "Fix") {
				if (this.anchors.length) {
					const points = this.anchors.filter(
						(point) => !this.sketch.relations?.some((r) => r.type === "fixed" && r.anchor.entityId === point.entityId && r.anchor.point === point.point)
					)
					if (!points.length) {
						this.status.textContent = "Selected points are already fixed."
						return
					}
					return this.edit(() => {
						for (const point of points)
							this.sketch.relations?.push({
								id: this.id("constraint"),
								type: "fixed",
								anchor: { ...point },
								position: copy(this.anchorPoint(point)),
								fixation: point.entityId
							})
					})
				}
				const entities = this.sketch.entities.filter((entity) => this.selected.includes(entity.id) && this.result.entityStates[entity.id] !== "fully-constrained")
				if (!entities.length) {
					this.status.textContent = "Selected entities are already fully constrained."
					return
				}
				return this.edit(() => {
					for (const entity of entities)
						for (const relation of fixedEntityRelations(entity, "fix", this.sketch.relations))
							this.sketch.relations?.push({ ...relation, id: this.id("constraint"), fixation: entity.id })
				})
			}

			const second = this.anchors[1] ?? (b ? this.defaultAnchor(b) : null)
			if (name === "Symmetric entities") {
				const other = need(b, "Select two entities, then Shift-select a symmetry line.")
				const axis = this.sketch.entities.find((entity) => entity.id === this.selected[2])
				if (this.selected.length !== 3 || axis?.type !== "line") throw Error("Select two entities first, then a separate symmetry line.")
				const source = other.type === "cornerRectangle" && a.type === "rectangle" ? other : a
				const target = source === a ? other : a
				if (mirrorSketchEntity(source, axis, target.id).type !== target.type) throw Error("Select matching native entity types, or a corner rectangle and a center rectangle.")
				return this.addRelation({ id, type: "mirror", a: source.id, b: target.id, symmetryLine: axis.id })
			}
			if (name === "Symmetric") {
				const other = need(second, "Select two points, then Shift-select a symmetry line.")
				if (anchor.entityId === other.entityId && anchor.point === other.point) throw Error("Choose two distinct points.")
				const axis = need(
					this.sketch.entities.find((e) => this.selected.includes(e.id) && e.id !== anchor.entityId && e.id !== other.entityId && e.type === "line"),
					"Shift-select a separate line as the symmetry axis."
				)
				return this.addRelation({ id, type: "symmetric", a: anchor, b: other, symmetryLine: axis.id })
			}
			if (name === "Coincident") return this.addRelation({ id, type: "coincident", a: anchor, b: need(second, "Select two points with Shift-click.") })
			if (["Distance", "X distance", "Y distance"].includes(name)) {
				const target = need(second, "Select two points.")
				const p = this.anchorPoint(anchor)
				const q = this.anchorPoint(target)
				const axis = name === "X distance" ? "x" : name === "Y distance" ? "y" : undefined
				return this.addRelation({ id, type: "distance", a: anchor, b: target, axis, value: axis ? q[axis] - p[axis] : Math.hypot(q.x - p.x, q.y - p.y) })
			}
			if (!b) throw Error("Shift-click a second entity.")
			if (name === "Equal" || name === "Parallel" || name === "Concentric" || name === "Collinear") {
				const group = this.selected.map((selectedId) => requireValue(this.sketch.entities.find((entity) => entity.id === selectedId)))
				if (name === "Concentric" && group.some((entity) => !["circle", "arc", "ellipse", "ellipticArc"].includes(entity.type)))
					throw Error("Select circles, arcs, ellipses or elliptic arcs for Concentric.")
				if ((name === "Parallel" || name === "Collinear") && group.some((entity) => entity.type !== "line")) throw Error(`Select only lines for ${name}.`)
				if (
					name === "Equal" &&
					!group.every((entity) =>
						a.type === "line"
							? entity.type === "line"
							: (["circle", "arc"].includes(a.type) && ["circle", "arc"].includes(entity.type)) ||
								(["ellipse", "ellipticArc"].includes(a.type) && ["ellipse", "ellipticArc"].includes(entity.type))
					)
				)
					throw Error("Select lines, circles and circular arcs, or ellipses and elliptic arcs as one compatible group for Equal.")
				const type = name === "Equal" ? "equal" : name === "Parallel" ? "parallel" : name === "Collinear" ? "collinear" : "concentric"
				const edges = (this.sketch.relations ?? []).filter(
					(r) => !r.reference && (r.type === type || (r.type === "sameEllipse" && (type === "equal" || type === "concentric")))
				)
				const connected = new Set([a.id])
				const expand = () => {
					let changed = true
					while (changed) {
						changed = false
						for (const edge of edges) {
							if (!("a" in edge) || typeof edge.a !== "string" || typeof edge.b !== "string") continue
							if (connected.has(edge.a) === connected.has(edge.b)) continue
							connected.add(edge.a)
							connected.add(edge.b)
							changed = true
						}
					}
				}
				expand()
				const missing = group.slice(1).filter((entity) => {
					if (connected.has(entity.id)) return false
					connected.add(entity.id)
					expand()
					return true
				})
				if (!missing.length) {
					this.status.textContent = `Selected entities are already connected by ${name} constraints.`
					return
				}
				return this.edit(() => {
					for (const entity of missing) {
						const relation: SketchRelation = {
							id: this.id("constraint"),
							type,
							a: a.id,
							b: entity.id
						}
						this.sketch.relations?.push(relation)
						this.selectedRelation = relation.id
					}
				})
			}
			if (name === "Tangent" && (a.type === "spline" || b.type === "spline" || ((a.type === "ellipticArc" || b.type === "ellipticArc") && this.anchors.length === 2))) {
				if (this.anchors.length !== 2 || !second || anchor.entityId === second.entityId) throw Error("Select two curve endpoints for tangency.")
				return this.addRelation({ id, type: "smoothJoin", a: anchor, b: second })
			}
			if (name === "Normal") {
				const line = a.type === "line" ? a : b.type === "line" ? b : null
				const curve = line === a ? b : a
				if (!line || !["line", "circle", "arc", "ellipse", "ellipticArc", "spline"].includes(curve.type))
					throw Error("Select a line endpoint and a line, circle, arc, ellipse, elliptic arc or spline.")
				const endpoint = this.anchors.find((p) => p.entityId === line.id) ?? { entityId: line.id, point: "p0" as const }
				return this.addRelation({ id, type: "normal", a: endpoint, b: curve.id })
			}
			if (name === "Midpoint" || name === "Point on curve") {
				if (anchor.entityId === b.id) throw Error("Select a point first, then Shift-click a different curve.")
				return this.addRelation({ id, type: name === "Midpoint" ? "midpoint" : "pointOnCurve", a: anchor, b: b.id })
			}
			if (name === "Internal tangent") {
				if ((a.type !== "circle" && a.type !== "arc") || (b.type !== "circle" && b.type !== "arc")) throw Error("Select two circles or arcs for internal tangency.")
				const outer = a.radius >= b.radius ? a : b
				const inner = outer === a ? b : a
				return this.addRelation({ id, type: "internalTangent", a: outer.id, b: inner.id })
			}
			if (name === "Radial distance") {
				if (a.type !== "circle" || b.type !== "circle") throw Error("Select two circles.")
				return this.addRelation({ id, type: "radiusDifference", a: a.id, b: b.id, value: a.radius - b.radius })
			}
			if (name === "Angle") {
				if (a.type !== "line" || b.type !== "line") throw Error("Select two lines.")
				const angle = ((Math.atan2(b.p1.y - b.p0.y, b.p1.x - b.p0.x) - Math.atan2(a.p1.y - a.p0.y, a.p1.x - a.p0.x)) * 180) / Math.PI
				return this.addRelation({ id, type: "angle", a: a.id, b: b.id, value: angle })
			}
			const types: Record<string, "concentric" | "equal" | "parallel" | "perpendicular" | "tangent"> = {
				Concentric: "concentric",
				Equal: "equal",
				Parallel: "parallel",
				Perpendicular: "perpendicular",
				Tangent: "tangent"
			}
			this.addRelation({ id, type: requireValue(types[name]), a: a.id, b: b.id })
		} catch (error) {
			this.status.textContent = error instanceof Error ? error.message : String(error)
		}
	}
	private addDimension(e: SketchEntity, type?: "width" | "height", reference = false) {
		if (e.type === "point" || e.type === "spline") {
			this.status.textContent = "Select two points to add a distance dimension."
			return
		}
		const id = this.id("dimension")
		if (e.type === "arc" || e.type === "polygon") this.addRelation({ id, reference, type: "radius", entityId: e.id, value: e.radius })
		else if (e.type === "circle") this.addRelation({ id, reference, type: "diameter", entityId: e.id, value: e.radius * 2 })
		else if (e.type === "line") this.addRelation({ id, reference, type: "length", entityId: e.id, value: Math.hypot(e.p1.x - e.p0.x, e.p1.y - e.p0.y) })
		else if (e.type === "ellipse" || e.type === "rectangle" || e.type === "ellipticArc")
			this.addRelation({ id, reference, type: type ?? "width", entityId: e.id, value: e[type ?? "width"] })
		else if (e.type === "cornerRectangle")
			this.addRelation({ id, reference, type: type ?? "width", entityId: e.id, value: Math.abs(type === "height" ? e.p1.y - e.p0.y : e.p1.x - e.p0.x) })
		else this.addRelation({ id, reference, type: "width", entityId: e.id, value: e.width })
	}
	private deleteSelected() {
		const relationId = this.selectedRelation
		if (relationId && this.sketch.relations?.some((r) => r.id === relationId)) {
			this.edit(() => {
				this.sketch.relations = this.sketch.relations?.filter((r) => r.id !== relationId)
				this.selectedRelation = null
			})
			return
		}
		if (!this.selected.length) return
		const removed = new Set(this.selected)
		this.edit(() => {
			this.sketch.entities = this.sketch.entities.filter((e) => !removed.has(e.id))
			this.sketch.relations = this.sketch.relations?.filter((r) => !sketchRelationEntityIds(r).some((id) => removed.has(id)))
			this.selected = []
			this.anchors = []
		})
	}
	private canSelect(entity: SketchEntity): boolean {
		switch (this.selectionFilter) {
			case "Construction":
				return !!entity.construction
			case "Non-construction":
				return !entity.construction
			case "Points":
				return entity.type === "point"
			case "Lines":
				return entity.type === "line"
			case "Curves":
				return ["circle", "arc", "ellipse", "capsule"].includes(entity.type)
			default:
				return true
		}
	}
	private select(id: string, shift: boolean, anchor?: SketchAnchor) {
		if (!this.sketch.entities.some((e) => e.id === id && this.canSelect(e))) return
		this.selected = shift ? [...new Set([...this.selected, id])] : [id]
		this.anchors = anchor ? (shift ? [...this.anchors, anchor] : [anchor]) : shift ? this.anchors : []
		this.selectedRelation = null
		this.renderProperties()
		this.draw()
	}
	private screen(p: Point2D): Point2D {
		return { x: this.width / 2 + (p.x - this.view.x) * this.view.scale, y: this.height / 2 - (p.y - this.view.y) * this.view.scale }
	}
	private world(x: number, y: number, snap = this.snapGrid || this.snapGeometry): Point2D {
		const rect = this.svg.getBoundingClientRect()
		const p = { x: (x - rect.left - this.width / 2) / this.view.scale + this.view.x, y: -(y - rect.top - this.height / 2) / this.view.scale + this.view.y }
		if (!snap) return p
		return sketchSnap(this.sketch.entities, p, this.view.scale, { grid: this.snapGrid, geometry: this.snapGeometry })?.position ?? p
	}
	private finishSpline(closed = false) {
		try {
			const entity = normalizeSpline({
				id: this.id("spline"),
				type: "spline",
				mode: this.tool === "Fit spline" ? "fit" : "control",
				points: this.splinePoints,
				...(closed ? { closed: true } : {}),
				construction: this.construction
			})
			this.edit(() => {
				this.insertEntity(entity)
				this.selected = [entity.id]
				this.selectedRelation = null
			})
			if (!this.sketch.entities.some((e) => e.id === entity.id)) return
			this.splinePoints = []
			this.tool = "Select"
			this.renderTools()
			this.renderProperties()
			this.draw()
		} catch (error) {
			this.status.textContent = error instanceof Error ? error.message : String(error)
		}
	}
	private pointerDown(e: PointerEvent) {
		if (e.button === 1 || e.button === 2) {
			e.preventDefault()
			this.drag = { before: copy(this.sketch), pan: { x: e.clientX, y: e.clientY }, center: { x: this.view.x, y: this.view.y } }
			this.svg.setPointerCapture?.(e.pointerId)
			return
		}
		if (e.button !== 0) return
		const p = this.world(e.clientX, e.clientY)
		if (["Trim", "Split", "Extend"].includes(this.tool)) {
			this.status.textContent = `${this.tool}: click the curve portion to edit.`
			return
		}
		if (this.tool === "Select" || this.tool === "Dimension") {
			if (this.tool === "Select") {
				const start = this.world(e.clientX, e.clientY, false)
				this.marquee = { start, end: start, additive: e.shiftKey, initial: [...this.selected], initialAnchors: [...this.anchors], initialRelation: this.selectedRelation }
				this.svg.setPointerCapture?.(e.pointerId)
			}
			if (!e.shiftKey) this.selected = []
			this.anchors = []
			this.selectedRelation = null
			this.renderProperties()
			this.draw()
			return
		}
		if (this.tool === "Fit spline" || this.tool === "Control spline") {
			const last = this.splinePoints.at(-1)
			if (this.tool === "Fit spline" && last && last.x === p.x && last.y === p.y) {
				this.status.textContent = "Choose a different fit point."
				return
			}
			if (this.splinePoints.length >= 4096) {
				this.status.textContent = "Spline point limit reached."
				return
			}
			this.splinePoints.push(p)
			this.renderTools()
			this.draw()
			this.status.textContent += ` · ${this.splinePoints.length} spline points. ${this.tool === "Fit spline" ? "Click fit points" : "Click start, two handles, then end; repeat handles and end"}; Enter or Finish spline completes the curve.`
			return
		}
		if (this.tool === "Point") {
			this.edit(() => {
				const entity: SketchEntity = { id: this.id("point"), type: "point", center: p, construction: this.construction }
				this.insertEntity(entity)
				this.selected = [entity.id]
				this.selectedRelation = null
			})
			return
		}
		if (this.tool === "Tangent arc") {
			if (!this.start) {
				const raw = this.world(e.clientX, e.clientY, false)
				const candidate = sketchSnap(this.sketch.entities, raw, this.view.scale, { endpointsOnly: true })
				if (!candidate) {
					this.status.textContent = "Tangent arc: click a line, arc, elliptic arc or spline endpoint first."
					return
				}
				const anchor = requireValue(candidate.anchor)
				try {
					const source = requireValue(this.sketch.entities.find((entity) => entity.id === anchor.entityId))
					sketchEndpointDirection(source, anchor.point)
				} catch (error) {
					this.status.textContent = error instanceof Error ? error.message : String(error)
					return
				}
				this.tangentStart = { entityId: anchor.entityId, point: anchor.point }
				this.start = candidate.position
				this.cursor = candidate.position
				this.draw()
			} else if (this.tangentStart) {
				try {
					const source = requireValue(this.sketch.entities.find((entity) => entity.id === this.tangentStart?.entityId))
					const arc = tangentArc(this.id("arc"), source, this.tangentStart.point, p)
					arc.construction = this.construction
					const reference = { ...this.tangentStart }
					this.edit(() => {
						this.insertEntity(arc)
						this.sketch.relations = (this.sketch.relations ?? []).filter(
							(r) => r.type !== "coincident" || !((r.a.entityId === arc.id && r.a.point === "p0") || (r.b.entityId === arc.id && r.b.point === "p0"))
						)
						this.sketch.relations.push({ id: this.id("smooth-join"), type: "smoothJoin", a: reference, b: { entityId: arc.id, point: "p0" } })
						this.selected = [arc.id]
						this.selectedRelation = null
					})
					this.start = null
					this.tangentStart = null
					this.draw()
				} catch (error) {
					this.status.textContent = error instanceof Error ? error.message : String(error)
				}
			}
			return
		}
		if (!this.start) {
			this.start = p
			this.cursor = p
			this.draw()
			return
		}
		if (
			this.tool === "Ellipse" ||
			this.tool === "3-point circle" ||
			this.tool === "3-point arc" ||
			this.tool === "Center arc" ||
			this.tool === "Slot" ||
			this.tool === "3-point rectangle" ||
			this.tool === "3-point center rectangle"
		) {
			if (!this.secondPoint) {
				if (Math.hypot(p.x - this.start.x, p.y - this.start.y) < 1e-9) {
					this.status.textContent = "Pick a different second point."
					return
				}
				this.secondPoint = p
				this.draw()
				return
			}
			try {
				const arc =
					this.tool === "Ellipse"
						? threePointEllipse(this.id("ellipse"), this.start, this.secondPoint, p)
						: this.tool === "3-point circle"
							? threePointCircle(this.id("circle"), this.start, this.secondPoint, p)
							: this.tool === "3-point center rectangle"
								? threePointCenterRectangle(this.id("rectangle"), this.start, this.secondPoint, p)
								: this.tool === "3-point rectangle"
									? threePointRectangle(this.id("rectangle"), this.start, this.secondPoint, p)
									: this.tool === "Slot"
										? threePointSlot(this.id("slot"), this.start, this.secondPoint, p)
										: this.tool === "Center arc"
											? centerPointArc(this.id("arc"), this.start, this.secondPoint, p, this.clockwiseArc)
											: threePointArc(this.id("arc"), this.start, this.secondPoint, p)
				arc.construction = this.construction
				const placementPoints = [this.start, this.secondPoint, p]
				this.edit(() => {
					this.insertEntity(arc, this.tool === "Center arc" || this.tool === "Ellipse" || this.tool === "3-point center rectangle")
					if (arc.type === "circle" && this.autoConstraints && this.snapGeometry) {
						const candidates = this.sketch.entities
							.filter((entity) => entity.id !== arc.id)
							.flatMap((entity) => entityAnchorNames(entity).map((point) => ({ entityId: entity.id, point, position: entityAnchorPoint(entity, point) })))
						const used = new Set<string>()
						for (const point of placementPoints) {
							const anchor = candidates.find((candidate) => Math.hypot(candidate.position.x - point.x, candidate.position.y - point.y) < 1e-6)
							if (!anchor || used.has(`${anchor.entityId}:${anchor.point}`)) continue
							used.add(`${anchor.entityId}:${anchor.point}`)
							this.sketch.relations ??= []
							this.sketch.relations.push({
								id: this.id("point-on-circle"),
								type: "pointOnCurve",
								a: { entityId: anchor.entityId, point: anchor.point },
								b: arc.id
							})
						}
					}
					this.selected = [arc.id]
				})
				this.start = null
				this.secondPoint = null
				this.draw()
			} catch (error) {
				this.status.textContent = error instanceof Error ? error.message : String(error)
			}
			return
		}
		const first = this.start
		const tool = this.tool
		this.start = null
		if (Math.hypot(p.x - first.x, p.y - first.y) < 1e-6) return
		this.edit(() => {
			const id = this.id(tool === "Center rectangle" ? "rectangle" : tool === "Diameter circle" ? "circle" : tool.toLowerCase())
			const construction = this.construction
			const entity: SketchEntity =
				tool === "Polygon"
					? { ...regularPolygon(id, first, p, this.polygonSides, this.circumscribedPolygon), construction }
					: tool === "Center rectangle"
						? { ...centerRectangle(id, first, p), construction }
						: tool === "Diameter circle"
							? {
									id,
									type: "circle",
									center: { x: (first.x + p.x) / 2, y: (first.y + p.y) / 2 },
									radius: Math.hypot(p.x - first.x, p.y - first.y) / 2,
									segments: 96,
									construction
								}
							: tool === "Circle"
								? { id, type: "circle", center: first, radius: Math.hypot(p.x - first.x, p.y - first.y), segments: 96, construction }
								: tool === "Rectangle"
									? { id, type: "cornerRectangle", p0: first, p1: p, construction }
									: { id, type: "line", p0: first, p1: p, construction }
			this.insertEntity(entity, tool === "Center rectangle" || tool === "Polygon" || tool === "Circle")
			if (tool === "Diameter circle" && this.autoConstraints && this.snapGeometry) {
				const used = new Set<string>()
				for (const position of [first, p]) {
					const match = this.sketch.entities
						.filter((other) => other.id !== id)
						.flatMap((other) => entityAnchorNames(other).map((point) => ({ entityId: other.id, point, position: entityAnchorPoint(other, point) })))
						.find((anchor) => Math.hypot(anchor.position.x - position.x, anchor.position.y - position.y) < 1e-6)
					if (!match || used.has(`${match.entityId}:${match.point}`)) continue
					used.add(`${match.entityId}:${match.point}`)
					this.sketch.relations ??= []
					this.sketch.relations.push({ id: this.id("point-on-circle"), type: "pointOnCurve", a: { entityId: match.entityId, point: match.point }, b: id })
				}
			}
			if (entity.type === "line" && this.autoConstraints && (Math.abs(first.x - p.x) < 1e-6 || Math.abs(first.y - p.y) < 1e-6))
				this.sketch.relations?.push({ id: this.id("constraint"), type: Math.abs(first.x - p.x) < 1e-6 ? "vertical" : "horizontal", entityId: id })
			this.selected = [id]
			this.selectedRelation = null
		})
		if (tool === "Line") this.start = p
	}
	private insertEntity(entity: SketchEntity, inferCenter = false) {
		const previous = [...this.sketch.entities]
		this.sketch.entities.push(entity)
		if (!this.autoConstraints || !this.snapGeometry) return
		if (
			entity.type !== "ellipse" &&
			entity.type !== "spline" &&
			entity.type !== "circle" &&
			entity.type !== "polygon" &&
			entity.type !== "point" &&
			entity.type !== "line" &&
			entity.type !== "arc" &&
			entity.type !== "capsule" &&
			entity.type !== "rectangle" &&
			entity.type !== "cornerRectangle"
		)
			return
		for (const point of (entity.type === "spline"
			? entityAnchorNames(entity)
			: entity.type === "polygon"
				? ["center", ...(this.circumscribedPolygon ? [] : ["vertex0"])]
				: entity.type === "circle"
					? inferCenter
						? ["center"]
						: []
					: entity.type === "point"
						? ["center"]
						: entity.type === "ellipse" || entity.type === "rectangle"
							? ["p0", "p1", "p2", "p3", ...(inferCenter ? ["center"] : [])]
							: entity.type === "capsule"
								? ["from", "to"]
								: inferCenter && entity.type === "arc"
									? ["p0", "p1", "center"]
									: ["p0", "p1"]) as SketchAnchor["point"][]) {
			const position = entityAnchorPoint(entity, point)
			const snap = this.snapGeometry ? sketchSnap(previous, position, this.view.scale, { grid: false }) : null
			if (snap?.kind === "origin" && Math.hypot(position.x, position.y) < 1e-6) {
				this.sketch.relations?.push({ id: this.id("constraint"), type: "fixed", anchor: { entityId: entity.id, point }, position: { x: 0, y: 0 } })
				continue
			}
			if (snap?.kind === "midpoint" && snap.curve && Math.hypot(snap.position.x - position.x, snap.position.y - position.y) < 1e-6) {
				this.sketch.relations?.push({ id: this.id("constraint"), type: "midpoint", a: { entityId: entity.id, point }, b: snap.curve })
				continue
			}
			if (snap?.kind === "intersection" && snap.curves && Math.hypot(snap.position.x - position.x, snap.position.y - position.y) < 1e-6) {
				for (const curve of snap.curves) this.sketch.relations?.push({ id: this.id("constraint"), type: "pointOnCurve", a: { entityId: entity.id, point }, b: curve })
				continue
			}
			for (const other of previous) {
				const match = entityAnchorNames(other).find((name) => {
					const p = entityAnchorPoint(other, name)
					return Math.hypot(p.x - position.x, p.y - position.y) < 1e-6
				})
				if (match) {
					this.sketch.relations?.push({ id: this.id("constraint"), type: "coincident", a: { entityId: entity.id, point }, b: { entityId: other.id, point: match } })
					break
				}
			}
		}
	}

	private pointerMove(e: PointerEvent) {
		if (this.marquee) {
			const selection = this.marquee
			selection.end = this.world(e.clientX, e.clientY, false)
			if (Math.hypot(selection.end.x - selection.start.x, selection.end.y - selection.start.y) * this.view.scale >= 4) {
				const box = {
					minX: Math.min(selection.start.x, selection.end.x),
					maxX: Math.max(selection.start.x, selection.end.x),
					minY: Math.min(selection.start.y, selection.end.y),
					maxY: Math.max(selection.start.y, selection.end.y)
				}
				const picked = this.sketch.entities
					.filter((entity) => this.canSelect(entity) && sketchEntityInBox(entity, box, selection.end.x < selection.start.x))
					.map((entity) => entity.id)
				this.selected = [...new Set([...(selection.additive ? selection.initial : []), ...picked])]
			} else this.selected = selection.additive ? [...selection.initial] : []
			this.draw()
			return
		}
		const p = this.world(e.clientX, e.clientY)
		if (this.drag?.pan && this.drag.center) {
			this.view.x = this.drag.center.x - (e.clientX - this.drag.pan.x) / this.view.scale
			this.view.y = this.drag.center.y + (e.clientY - this.drag.pan.y) / this.view.scale
			this.draw()
			return
		}
		if (this.drag?.groupScale) {
			const group = this.drag.groupScale
			const raw = this.world(e.clientX, e.clientY, false)
			let factor = ((raw.x - group.center.x) * group.vector.x + (raw.y - group.center.y) * group.vector.y) / (group.vector.x ** 2 + group.vector.y ** 2)
			if (e.shiftKey) factor = Math.round(factor * 10) / 10
			if (!Number.isFinite(factor) || factor <= 1e-4) {
				this.status.textContent = "Scale factor must remain positive."
				return
			}
			try {
				const scaled =
					Math.abs(factor - 1) > 1e-9
						? scaleSketchSelection(this.drag.before, group.selected, group.center, factor)
						: { sketch: copy(this.drag.before), removedRelations: [] }
				const result = solveSketch(scaled.sketch.entities, scaled.sketch.relations ?? [])
				if (result.status === "conflicting") throw Error("The group cannot scale to this size.")
				this.sketch = { ...scaled.sketch, entities: result.entities }
				this.result = result
				group.removed = scaled.removedRelations
				group.applied = factor
				this.draw()
				this.status.textContent += ` · Scale ${Number(factor.toFixed(3))}×`
				if (group.removed.length) this.status.textContent += ` · Will detach: ${group.removed.join(", ")}`
			} catch (error) {
				this.status.textContent = error instanceof Error ? error.message : String(error)
			}
			return
		}
		if (this.drag?.groupRotate) {
			const group = this.drag.groupRotate
			const raw = this.world(e.clientX, e.clientY, false)
			if (Math.hypot(raw.x - group.center.x, raw.y - group.center.y) * this.view.scale < 4) return
			const angle = Math.atan2(raw.y - group.center.y, raw.x - group.center.x)
			group.value = circularPatternDragAngle(group.value, group.last, angle)
			group.last = angle
			const degrees = e.shiftKey ? Math.round(group.value / 15) * 15 : group.value
			try {
				const rotated =
					Math.abs(degrees % 360) > 1e-9
						? rotateSketchSelection(this.drag.before, group.selected, group.center, degrees)
						: { sketch: copy(this.drag.before), removedRelations: [] }
				const result = solveSketch(rotated.sketch.entities, rotated.sketch.relations ?? [])
				if (result.status === "conflicting") throw Error("The group cannot rotate to this position.")
				this.sketch = { ...rotated.sketch, entities: result.entities }
				this.result = result
				group.removed = rotated.removedRelations
				group.applied = degrees
				this.draw()
				this.status.textContent += ` · Rotate ${Number(degrees.toFixed(2))}°`
				if (group.removed.length) this.status.textContent += ` · Will detach: ${group.removed.join(", ")}`
			} catch (error) {
				this.status.textContent = error instanceof Error ? error.message : String(error)
			}
			return
		}
		if (this.drag?.groupMove) {
			const group = this.drag.groupMove
			const raw = this.world(e.clientX, e.clientY, false)
			const delta = { x: raw.x - group.start.x, y: raw.y - group.start.y }
			if (e.shiftKey) {
				if (Math.abs(delta.x) >= Math.abs(delta.y)) delta.y = 0
				else delta.x = 0
			}
			if (this.snapGrid) {
				delta.x = Math.round(delta.x)
				delta.y = Math.round(delta.y)
			}
			try {
				const translated = delta.x || delta.y ? translateSketchSelection(this.drag.before, group.selected, delta) : { sketch: copy(this.drag.before), removedRelations: [] }
				const result = solveSketch(translated.sketch.entities, translated.sketch.relations ?? [])
				if (result.status === "conflicting") throw Error("The group cannot move to this position.")
				this.sketch = { ...translated.sketch, entities: result.entities }
				this.result = result
				group.removed = translated.removedRelations
				this.draw()
				this.status.textContent += ` · Move X ${Number(delta.x.toFixed(3))} mm, Y ${Number(delta.y.toFixed(3))} mm`
				if (group.removed.length) this.status.textContent += ` · Will detach: ${group.removed.join(", ")}`
			} catch (error) {
				this.status.textContent = error instanceof Error ? error.message : String(error)
			}
			return
		}
		if (this.drag?.patternStep) {
			const drag = this.drag.patternStep
			const original = this.drag.before
			const relation = original.relations?.find((r) => r.id === drag.id)
			if (relation?.type === "linearPattern") {
				const source = requireValue(original.entities.find((entity) => entity.id === relation.sources[0]))
				const origin = entityAnchorPoint(source, this.defaultAnchor(source).point)
				const { columns, rows, rowStep } = patternLayout(relation)
				let step = { x: p.x - origin.x, y: p.y - origin.y }
				if (e.shiftKey) {
					const direction = drag.axis === "column" ? relation.step : rowStep
					const t = (step.x * direction.x + step.y * direction.y) / (direction.x * direction.x + direction.y * direction.y)
					step = { x: direction.x * t, y: direction.y * t }
				}
				try {
					const next = resizeLinearSketchPattern(
						original,
						relation.id,
						columns,
						drag.axis === "column" ? step : relation.step,
						rows,
						drag.axis === "row" ? step : rowStep
					).sketch
					const result = solveSketch(next.entities, next.relations ?? [])
					if (result.status === "conflicting") this.status.textContent = "That pattern spacing conflicts with existing constraints."
					else {
						this.sketch = { ...next, entities: result.entities }
						this.result = result
						this.draw()
					}
				} catch (error) {
					this.status.textContent = error instanceof Error ? error.message : String(error)
				}
			}
			return
		}
		if (this.drag?.patternAngle) {
			const drag = this.drag.patternAngle
			const original = this.drag.before
			const relation = original.relations?.find((r) => r.id === drag.id)
			if (relation?.type === "circularPattern") {
				const center = circularPatternCenter(relation, original.entities)
				const point = this.world(e.clientX, e.clientY, false)
				if (Math.hypot(point.x - center.x, point.y - center.y) * this.view.scale < 4) return
				const pointerAngle = Math.atan2(point.y - center.y, point.x - center.x)
				drag.value = circularPatternDragAngle(drag.value, drag.last, pointerAngle)
				drag.last = pointerAngle
				let angle = e.shiftKey ? Math.round(drag.value / 15) * 15 : drag.value
				if (Math.abs(angle) > 359.5) angle = Math.sign(angle) * 360
				if (Math.abs(angle) < 0.1) angle = Math.sign(angle || relation.angle) * 0.1
				try {
					const next = resizeCircularSketchPattern(original, relation.id, relation.instances.length + 1, center, angle).sketch
					const result = solveSketch(next.entities, next.relations ?? [])
					if (result.status === "conflicting") this.status.textContent = "That pattern angle conflicts with existing constraints."
					else {
						this.sketch = { ...next, entities: result.entities }
						this.result = result
						this.draw()
					}
				} catch (error) {
					this.status.textContent = error instanceof Error ? error.message : String(error)
				}
			}
			return
		}
		if (this.drag?.patternCenter) {
			const original = this.drag.before
			const relation = original.relations?.find((r) => r.id === this.drag?.patternCenter)
			if (relation?.type === "circularPattern") {
				try {
					const next = resizeCircularSketchPattern(original, relation.id, relation.instances.length + 1, p, relation.angle).sketch
					const result = solveSketch(next.entities, next.relations ?? [])
					if (result.status === "conflicting") this.status.textContent = "That pattern center conflicts with existing constraints."
					else {
						this.sketch = { ...next, entities: result.entities }
						this.result = result
						this.draw()
					}
				} catch (error) {
					this.status.textContent = error instanceof Error ? error.message : String(error)
				}
			}
			return
		}
		if (this.drag?.anchor) {
			const original = this.drag.before
			try {
				const result = solveSketch(original.entities, [...(original.relations ?? []), { id: "drag-target", type: "fixed", anchor: this.drag.anchor, position: p }])
				if (result.status !== "conflicting") {
					this.sketch.entities = result.entities
					this.result = solveSketch(result.entities, this.sketch.relations ?? [])
					this.draw()
				} else this.status.textContent = "That point is constrained. Edit its driving dimension instead."
			} catch {}
			return
		}
		if (this.drag?.label) {
			this.svg.setPointerCapture?.(e.pointerId)
			const r = this.sketch.relations?.find((r) => r.id === this.drag?.label)
			if (r) r.labelPosition = p
			this.draw()
			return
		}
		this.cursor = p
		const placing = !["Select", "Dimension", "Trim", "Split", "Extend"].includes(this.tool)
		this.hoverSnap =
			placing && (this.snapGrid || this.snapGeometry || (this.tool === "Tangent arc" && !this.start))
				? sketchSnap(this.sketch.entities, this.world(e.clientX, e.clientY, false), this.view.scale, {
						endpointsOnly: this.tool === "Tangent arc" && !this.start,
						grid: this.snapGrid,
						geometry: this.snapGeometry
					})
				: null
		if (this.start || placing) this.draw()
	}
	private cancelDrag() {
		if (this.marquee) {
			this.selected = this.marquee.initial
			this.anchors = this.marquee.initialAnchors
			this.selectedRelation = this.marquee.initialRelation
			this.marquee = null
			this.renderProperties()
			this.draw()
		}
		if (!this.drag) return
		this.sketch = this.drag.before
		if (this.drag.pan && this.drag.center) {
			this.view.x = this.drag.center.x
			this.view.y = this.drag.center.y
		}
		this.drag = null
		this.result = solveSketch(this.sketch.entities, this.sketch.relations ?? [])
		this.renderTools()
		this.renderProperties()
		this.draw()
	}
	private pointerUp() {
		if (this.marquee) {
			this.marquee = null
			this.renderProperties()
			this.draw()
			return
		}
		if (this.drag && !this.drag.pan && JSON.stringify(this.drag.before) !== JSON.stringify(this.sketch)) {
			this.undo.push(this.drag.before)
			this.redo = []
			this.renderTools()
			this.renderProperties()
		}
		const removed = this.drag?.groupMove?.removed ?? this.drag?.groupRotate?.removed ?? this.drag?.groupScale?.removed ?? []
		this.drag = null
		if (removed.length) this.status.textContent += ` · Detached external constraints: ${removed.join(", ")}`
	}
	private key(e: KeyboardEvent) {
		if (this.disposed) return
		if (["INPUT", "SELECT", "TEXTAREA"].includes((e.target as HTMLElement).tagName)) return
		if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a" && !e.altKey) {
			e.preventDefault()
			e.stopPropagation()
			this.cancelDrag()
			this.start = null
			this.splinePoints = []
			this.secondPoint = null
			this.splitCircleStart = null
			this.hoverSnap = null
			this.tool = "Select"
			this.selected = this.sketch.entities.filter((entity) => this.canSelect(entity)).map((entity) => entity.id)
			this.anchors = []
			this.selectedRelation = null
			this.renderTools()
			this.renderProperties()
			this.draw()
			return
		}
		if (e.key.toLowerCase() === "s" && !e.ctrlKey && !e.metaKey && !e.altKey) {
			e.preventDefault()
			e.stopPropagation()
			this.openToolPalette()
			return
		}
		if (e.key === "Enter" && (this.tool === "Fit spline" || this.tool === "Control spline")) {
			e.preventDefault()
			this.finishSpline()
			return
		}
		if (e.key === "Escape") {
			e.preventDefault()
			e.stopPropagation()
			const cancellingGesture = !!this.drag || !!this.marquee || !!this.start || !!this.splinePoints.length
			this.tangentStart = null
			this.cancelDrag()
			this.start = null
			this.splinePoints = []
			this.secondPoint = null
			this.splitCircleStart = null
			this.tool = "Select"
			this.hoverSnap = null
			if (!cancellingGesture) {
				this.selected = []
				this.anchors = []
				this.selectedRelation = null
			}
			this.renderTools()
			this.renderProperties()
			this.draw()
			return
		}
		if (e.key === "Delete" || e.key === "Backspace") {
			e.preventDefault()
			this.deleteSelected()
		}
		if ((e.ctrlKey || e.metaKey) && ["z", "y"].includes(e.key.toLowerCase())) {
			e.preventDefault()
			e.stopPropagation()
			this.history(e.shiftKey || e.key.toLowerCase() === "y")
			return
		}
		if (e.key.toLowerCase() === "d") {
			this.tool = "Dimension"
			this.renderTools()
		}
		if (e.key.toLowerCase() === "f") this.fit()
	}
	private history(redo: boolean) {
		const from = redo ? this.redo : this.undo
		const to = redo ? this.undo : this.redo
		const next = from.pop()
		if (!next) return
		to.push(copy(this.sketch))
		this.sketch = next
		this.result = solveSketch(next.entities, next.relations ?? [])
		this.selected = []
		this.selectedRelation = null
		this.renderTools()
		this.renderProperties()
		this.draw()
	}
	private points(e: SketchEntity): Point2D[] {
		if (e.type === "ellipticArc") return sampleEllipticArc(e)
		if (e.type === "spline") return sampleSpline(e, 0.01).map((s) => s.point)
		return e.type === "arc"
			? arcPoints(e)
			: e.type === "line"
				? [e.p0, e.p1]
				: e.type === "cornerRectangle"
					? [e.p0, { x: e.p1.x, y: e.p0.y }, e.p1, { x: e.p0.x, y: e.p1.y }]
					: primitivePoints(e)
	}
	private fit() {
		const rect = this.svg.getBoundingClientRect()
		if (rect.width > 0 && rect.height > 0) this.initialFitPending = false
		const points = [...this.sketch.entities.flatMap((e) => this.points(e)), ...(this.showSupport ? this.supportEdges.flat() : [])]
		if (points.length) {
			const xs = points.map((p) => p.x)
			const ys = points.map((p) => p.y)
			this.view.x = (Math.min(...xs) + Math.max(...xs)) / 2
			this.view.y = (Math.min(...ys) + Math.max(...ys)) / 2
			this.view.scale = Math.min(
				((rect.width || 1000) * 0.72) / Math.max(20, Math.max(...xs) - Math.min(...xs)),
				((rect.height || 700) * 0.72) / Math.max(20, Math.max(...ys) - Math.min(...ys))
			)
		}
		this.draw()
	}
	private node<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string): SVGElementTagNameMap[K] {
		const n = document.createElementNS(NS, tag)
		for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, String(v))
		if (text) n.textContent = text
		this.svg.append(n)
		return n
	}
	private draw() {
		if (this.disposed) return
		const rect = this.svg.getBoundingClientRect()
		this.width = rect.width || 1000
		this.height = rect.height || 700
		this.svg.setAttribute("viewBox", `0 0 ${this.width} ${this.height}`)
		this.svg.replaceChildren()
		const spacing = 10 ** Math.floor(Math.log10(60 / this.view.scale))
		const step = spacing * this.view.scale
		const origin = this.screen({ x: 0, y: 0 })
		for (let x = ((origin.x % step) + step) % step; x < this.width; x += step) this.node("line", { x1: x, x2: x, y1: 0, y2: this.height, stroke: "#eef1f5" })
		for (let y = ((origin.y % step) + step) % step; y < this.height; y += step) this.node("line", { x1: 0, x2: this.width, y1: y, y2: y, stroke: "#eef1f5" })
		this.node("line", { x1: 0, x2: this.width, y1: origin.y, y2: origin.y, stroke: "#dc9b9b" })
		this.node("line", { x1: origin.x, x2: origin.x, y1: 0, y2: this.height, stroke: "#9bb7dc" })
		this.node("circle", { cx: origin.x, cy: origin.y, r: 3, fill: "#334155" })
		if (this.showSupport)
			for (const edge of this.supportEdges) {
				const points = edge.map((point) => this.screen(point))
				this.node("path", {
					d: points.map((point, i) => `${i ? "L" : "M"} ${point.x} ${point.y}`).join(" "),
					fill: "none",
					stroke: "#94a3b8",
					"stroke-width": 1.5,
					"stroke-dasharray": "4 3",
					"pointer-events": "none",
					"aria-hidden": "true",
					"data-support-edge": ""
				})
			}
		try {
			const topology = materializeSketch(this.sketch)
			const d = topology.loops
				.map(
					(loop) =>
						`M ${loop.vertexIndices
							.map((i) => this.screen(requireValue(topology.vertices[i])))
							.map((p) => `${p.x} ${p.y}`)
							.join(" L ")} Z`
				)
				.join(" ")
			this.node("path", { d, fill: "#dce2e8", "fill-rule": "evenodd", "pointer-events": "none" })
		} catch {
			/* Keep conflicting geometry editable even when it cannot produce a region. */
		}
		if (this.splinePoints.length) {
			const points = this.splinePoints.map((p) => this.screen(p))
			this.node("path", {
				d: points.map((p, i) => `${i ? "L" : "M"} ${p.x} ${p.y}`).join(" "),
				fill: "none",
				stroke: "#94a3b8",
				"stroke-dasharray": "4 3",
				"pointer-events": "none",
				"data-spline-preview": "handles"
			})
			for (const p of points) this.node("circle", { cx: p.x, cy: p.y, r: 3, fill: "#2563eb", "pointer-events": "none" })
			try {
				const spline = normalizeSpline({ id: "preview", type: "spline", mode: this.tool === "Fit spline" ? "fit" : "control", points: this.splinePoints })
				const curve = sampleSpline(spline, 0.01).map((s) => this.screen(s.point))
				this.node("path", {
					d: curve.map((p, i) => `${i ? "L" : "M"} ${p.x} ${p.y}`).join(" "),
					fill: "none",
					stroke: "#2563eb",
					"pointer-events": "none",
					"data-spline-preview": "curve"
				})
			} catch {
				/* Incomplete control groups show their handles until the next end point. */
			}
		}
		for (const e of this.sketch.entities) {
			const points = this.points(e).map((p) => this.screen(p))
			const chosen = this.selected.includes(e.id)
			if (chosen && e.type === "spline" && e.mode === "control") {
				const controls = e.points.map((point) => this.screen(point))
				this.node("path", {
					d: controls.map((point, index) => `${index ? "L" : "M"} ${point.x} ${point.y}`).join(" "),
					fill: "none",
					stroke: "#64748b",
					"stroke-width": 1,
					"stroke-dasharray": "4 3",
					"pointer-events": "none",
					"aria-hidden": "true",
					"data-spline-control-polygon": e.id
				})
			}
			const state = this.result.entityStates[e.id] ?? "unknown"
			const stroke = chosen ? "#f59e0b" : state === "conflicting" ? "#dc2626" : state === "fully-constrained" ? "#172033" : state === "unknown" ? "#64748b" : "#2563eb"
			const path = this.node("path", {
				d:
					e.type === "point"
						? `M ${requireValue(points[0]).x - 4} ${requireValue(points[0]).y} h 8 M ${requireValue(points[0]).x} ${requireValue(points[0]).y - 4} v 8`
						: `M ${points.map((p) => `${p.x} ${p.y}`).join(" L ")}${e.type === "line" || e.type === "arc" || e.type === "spline" || e.type === "ellipticArc" ? "" : " Z"}`,
				fill: "none",
				stroke,
				"stroke-width": chosen ? 3 : 2,
				"stroke-dasharray": e.construction && e.type !== "point" ? "9 5" : "",
				"data-entity-id": e.id,
				"data-constraint-state": state,
				"aria-label": `${e.type} ${e.id}${e.construction ? " construction" : ""}`,
				role: "button"
			})
			path.style.cursor = this.canSelect(e) ? "pointer" : "default"
			path.setAttribute("pointer-events", this.canSelect(e) ? "stroke" : "none")
			path.setAttribute("aria-disabled", String(!this.canSelect(e)))
			path.onpointerdown = (event) => {
				if (!this.canSelect(e)) return
				if (["Trim", "Split", "Extend"].includes(this.tool)) {
					event.stopPropagation()
					if (this.splitCircleStart && this.splitCircleStart.id !== e.id) this.splitCircleStart = null
					if (this.tool === "Split" && (e.type === "circle" || e.type === "ellipse") && this.splitCircleStart?.id !== e.id) {
						this.splitCircleStart = { id: e.id, point: this.world(event.clientX, event.clientY, false) }
						this.status.textContent = `Split ${e.type}: click a second point on this ${e.type}.`
						return
					}
					let removed: string[] = []
					this.edit(() => {
						const result = editSketchCurve(
							this.sketch,
							e.id,
							this.splitCircleStart?.point ?? this.world(event.clientX, event.clientY, false),
							this.tool as SketchEditOperation,
							this.splitCircleStart ? this.world(event.clientX, event.clientY, false) : undefined
						)
						this.sketch = result.sketch
						this.splitCircleStart = null
						removed = result.removedRelations
						this.selected = this.sketch.entities.some((entity) => entity.id === e.id) ? [e.id] : []
						this.selectedRelation = null
					})
					if (removed.length) this.status.textContent += ` · Removed constraints: ${removed.join(", ")}`
					return
				}
				if (this.tool !== "Select" && this.tool !== "Dimension") return
				event.stopPropagation()
				this.select(e.id, event.shiftKey)
				if (this.tool === "Dimension") this.addDimension(e)
			}
			if (chosen)
				for (const name of entityAnchorNames(e)) {
					const anchor = { entityId: e.id, point: name } as SketchAnchor
					const p = this.screen(this.anchorPoint(anchor))
					const handle = this.node("circle", {
						cx: p.x,
						cy: p.y,
						r: 5,
						fill: "white",
						stroke: "#d97706",
						"stroke-width": 2,
						role: "button",
						"aria-label": `${e.id} ${name}`
					})
					handle.onpointerdown = (event) => {
						if (["Trim", "Split", "Extend"].includes(this.tool)) {
							path.onpointerdown?.call(path, event)
							return
						}
						if (this.tool !== "Select") return
						event.stopPropagation()
						this.select(e.id, event.shiftKey, anchor)
						this.drag = { anchor, before: copy(this.sketch) }
						this.svg.setPointerCapture?.(event.pointerId)
					}
				}
		}
		for (const r of this.sketch.relations ?? [])
			if ("value" in r || this.showConstraints || r.id === this.selectedRelation || this.result.conflicts.includes(r.id) || this.result.redundantRelations.includes(r.id))
				this.drawRelation(r)
		if (this.start && this.cursor) {
			const a = this.screen(this.start)
			const b = this.screen(this.cursor)
			if (this.tool === "Ellipse" && this.secondPoint) {
				try {
					const ellipse = threePointEllipse("preview", this.start, this.secondPoint, this.cursor)
					const points = primitivePoints(ellipse).map((point) => this.screen(point))
					this.node("path", {
						d: `M ${points.map((point) => `${point.x} ${point.y}`).join(" L ")} Z`,
						fill: "none",
						stroke: "#60a5fa",
						"pointer-events": "none",
						"data-preview": "ellipse"
					})
					for (const point of [this.secondPoint, entityAnchorPoint(ellipse, "p1")]) {
						const end = this.screen(point)
						this.node("line", { x1: a.x, y1: a.y, x2: end.x, y2: end.y, stroke: "#60a5fa", "stroke-dasharray": "3 4", "pointer-events": "none" })
					}
				} catch {}
			} else if (this.tool === "Polygon") {
				try {
					const points = primitivePoints(regularPolygon("preview", this.start, this.cursor, this.polygonSides, this.circumscribedPolygon)).map((point) =>
						this.screen(point)
					)
					this.node("path", {
						d: `M ${points.map((point) => `${point.x} ${point.y}`).join(" L ")} Z`,
						fill: "none",
						stroke: "#60a5fa",
						"pointer-events": "none",
						"data-preview": "polygon"
					})
				} catch {}
			} else if (this.tool === "3-point circle" && this.secondPoint) {
				try {
					const circle = threePointCircle("preview", this.start, this.secondPoint, this.cursor)
					const center = this.screen(circle.center)
					this.node("circle", {
						cx: center.x,
						cy: center.y,
						r: circle.radius * this.view.scale,
						fill: "none",
						stroke: "#60a5fa",
						"pointer-events": "none",
						"data-preview": "three-point-circle"
					})
				} catch {}
			} else if (this.tool === "Tangent arc" && this.tangentStart) {
				try {
					const source = requireValue(this.sketch.entities.find((entity) => entity.id === this.tangentStart?.entityId))
					const points = arcPoints(tangentArc("preview", source, this.tangentStart.point, this.cursor)).map((point) => this.screen(point))
					this.node("path", {
						d: `M ${points.map((point) => `${point.x} ${point.y}`).join(" L ")}`,
						fill: "none",
						stroke: "#60a5fa",
						"pointer-events": "none",
						"data-preview": "tangent-arc"
					})
				} catch {}
			} else if (this.tool === "Center rectangle" || ((this.tool === "3-point rectangle" || this.tool === "3-point center rectangle") && this.secondPoint)) {
				try {
					const rectangle =
						this.tool === "Center rectangle"
							? centerRectangle("preview", this.start, this.cursor)
							: this.tool === "3-point center rectangle"
								? threePointCenterRectangle("preview", this.start, requireValue(this.secondPoint), this.cursor)
								: threePointRectangle("preview", this.start, requireValue(this.secondPoint), this.cursor)
					const points = primitivePoints(rectangle).map((p) => this.screen(p))
					this.node("path", {
						d: `M ${points.map((p) => `${p.x} ${p.y}`).join(" L ")} Z`,
						fill: "none",
						stroke: "#60a5fa",
						"pointer-events": "none",
						"data-preview": "rectangle"
					})
				} catch {}
			} else if (this.tool === "Slot" && this.secondPoint) {
				try {
					const points = primitivePoints(threePointSlot("preview", this.start, this.secondPoint, this.cursor)).map((p) => this.screen(p))
					this.node("path", {
						d: `M ${points.map((p) => `${p.x} ${p.y}`).join(" L ")} Z`,
						fill: "none",
						stroke: "#60a5fa",
						"pointer-events": "none",
						"stroke-dasharray": "5 4",
						"data-preview": "slot"
					})
				} catch {}
			} else if ((this.tool === "3-point arc" || this.tool === "Center arc") && this.secondPoint) {
				try {
					const preview =
						this.tool === "Center arc"
							? centerPointArc("preview", this.start, this.secondPoint, this.cursor, this.clockwiseArc)
							: threePointArc("preview", this.start, this.secondPoint, this.cursor)
					if (this.tool === "Center arc") {
						this.node("circle", { cx: a.x, cy: a.y, r: 3, fill: "#60a5fa", "pointer-events": "none" })
						for (const fraction of [0, 1]) {
							const endpoint = this.screen(arcPoint(preview, fraction))
							this.node("line", { x1: a.x, y1: a.y, x2: endpoint.x, y2: endpoint.y, stroke: "#60a5fa", "stroke-dasharray": "3 4", "pointer-events": "none" })
						}
					}
					const points = arcPoints(preview).map((p) => this.screen(p))
					this.node("path", { d: `M ${points.map((p) => `${p.x} ${p.y}`).join(" L ")}`, fill: "none", stroke: "#60a5fa", "pointer-events": "none" })
				} catch {}
			} else if (this.tool === "Diameter circle")
				this.node("circle", { cx: (a.x + b.x) / 2, cy: (a.y + b.y) / 2, r: Math.hypot(b.x - a.x, b.y - a.y) / 2, fill: "none", stroke: "#60a5fa", "stroke-dasharray": "5 4" })
			else if (this.tool === "Circle") this.node("circle", { cx: a.x, cy: a.y, r: Math.hypot(b.x - a.x, b.y - a.y), fill: "none", stroke: "#60a5fa", "stroke-dasharray": "5 4" })
			else if (this.tool === "Rectangle")
				this.node("rect", { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y), fill: "none", stroke: "#60a5fa" })
			else this.node("line", { x1: a.x, y1: a.y, x2: b.x, y2: b.y, stroke: "#60a5fa" })
		}
		if (this.marquee) {
			const a = this.screen(this.marquee.start)
			const b = this.screen(this.marquee.end)
			const crossing = this.marquee.end.x < this.marquee.start.x
			this.node("rect", {
				x: Math.min(a.x, b.x),
				y: Math.min(a.y, b.y),
				width: Math.abs(b.x - a.x),
				height: Math.abs(b.y - a.y),
				fill: crossing ? "#22c55e22" : "#3b82f622",
				stroke: crossing ? "#16a34a" : "#2563eb",
				"stroke-dasharray": crossing ? "5 4" : "",
				"pointer-events": "none",
				"data-selection-box": crossing ? "crossing" : "contained"
			})
		}

		if (this.tool === "Select" && this.selected.length && !this.selectedRelation && !this.marquee) {
			const points = this.sketch.entities
				.filter((e) => this.selected.includes(e.id))
				.flatMap((e) => this.points(e))
				.map((p) => this.screen(p))
			if (points.length) {
				const x = (Math.min(...points.map((p) => p.x)) + Math.max(...points.map((p) => p.x))) / 2
				const y = Math.min(...points.map((p) => p.y)) - 24
				const handle = this.node("circle", {
					cx: x,
					cy: y,
					r: 10,
					fill: "white",
					stroke: "#2563eb",
					"stroke-width": 2,
					role: "button",
					tabindex: 0,
					"aria-label": "Move selected group",
					"data-group-move-handle": "true"
				})
				handle.style.cursor = "move"
				handle.onpointerdown = (event) => {
					if (event.button !== 0) return
					event.stopPropagation()
					this.drag = { before: copy(this.sketch), groupMove: { start: this.world(event.clientX, event.clientY, false), selected: [...this.selected], removed: [] } }
					this.svg.setPointerCapture?.(event.pointerId)
				}
				handle.onkeydown = (event) => {
					if (event.key !== "Enter") return
					event.preventDefault()
					event.stopPropagation()
					this.properties.querySelector<HTMLInputElement>('[aria-label="Move X (mm)"]')?.focus()
				}
				this.node("path", { d: `M ${x - 5} ${y} h 10 M ${x} ${y - 5} v 10`, stroke: "#2563eb", "pointer-events": "none" })
				this.node("text", { x: x + 15, y: y + 4, fill: "#2563eb", "font-size": 12, "pointer-events": "none" }, "Move")
				const pivot = this.screen(this.drag?.groupRotate?.center ?? this.rotationCenter)
				const group = this.drag?.groupRotate
				const turn = group ? group.initial + (group.applied * Math.PI) / 180 : 0
				const rx = group ? pivot.x + group.radius * this.view.scale * Math.cos(turn) : Math.max(...points.map((p) => p.x)) + 72
				const ry = group ? pivot.y - group.radius * this.view.scale * Math.sin(turn) : y
				this.node("line", { x1: pivot.x, y1: pivot.y, x2: rx, y2: ry, stroke: "#7c3aed", "stroke-dasharray": "4 4", "pointer-events": "none" })
				this.node("circle", { cx: pivot.x, cy: pivot.y, r: 4, fill: "none", stroke: "#7c3aed", "pointer-events": "none", "data-group-rotation-pivot": "true" })
				const rotation = this.node("circle", {
					cx: rx,
					cy: ry,
					r: 10,
					fill: "white",
					stroke: "#7c3aed",
					"stroke-width": 2,
					role: "button",
					tabindex: 0,
					"aria-label": "Rotate selected group",
					"data-group-rotate-handle": "true"
				})
				rotation.style.cursor = "grab"
				rotation.onpointerdown = (event) => {
					if (event.button !== 0) return
					event.stopPropagation()
					const raw = this.world(event.clientX, event.clientY, false)
					const center = { ...this.rotationCenter }
					const initial = Math.atan2(raw.y - center.y, raw.x - center.x)
					const radius = Math.hypot(raw.x - center.x, raw.y - center.y)
					if (radius * this.view.scale < 4) return
					this.drag = {
						before: copy(this.sketch),
						groupRotate: { center, initial, last: initial, radius, value: 0, applied: 0, selected: [...this.selected], removed: [] }
					}
					this.svg.setPointerCapture?.(event.pointerId)
				}
				rotation.onkeydown = (event) => {
					if (event.key !== "Enter") return
					event.preventDefault()
					event.stopPropagation()
					this.properties.querySelector<HTMLInputElement>('[aria-label="Rotate angle (degrees)"]')?.focus()
				}
				this.node("text", { x: rx + 15, y: ry + 4, fill: "#7c3aed", "font-size": 12, "pointer-events": "none" }, "Rotate")
				const sizing = this.drag?.groupScale
				const scalePivot = this.screen(sizing?.center ?? this.scaleCenter)
				const sx = sizing ? scalePivot.x + sizing.vector.x * sizing.applied * this.view.scale : Math.max(...points.map((p) => p.x)) + 24
				const sy = sizing ? scalePivot.y - sizing.vector.y * sizing.applied * this.view.scale : Math.max(...points.map((p) => p.y)) + 24
				this.node("line", { x1: scalePivot.x, y1: scalePivot.y, x2: sx, y2: sy, stroke: "#15803d", "stroke-dasharray": "4 4", "pointer-events": "none" })
				this.node("rect", {
					x: scalePivot.x - 4,
					y: scalePivot.y - 4,
					width: 8,
					height: 8,
					fill: "none",
					stroke: "#15803d",
					"pointer-events": "none",
					"data-group-scale-pivot": "true",
					"data-center-x": scalePivot.x,
					"data-center-y": scalePivot.y
				})
				const scaleHandle = this.node("rect", {
					x: sx - 9,
					y: sy - 9,
					width: 18,
					height: 18,
					fill: "white",
					stroke: "#15803d",
					"stroke-width": 2,
					role: "button",
					tabindex: 0,
					"aria-label": "Scale selected group",
					"data-group-scale-handle": "true"
				})
				scaleHandle.style.cursor = "nwse-resize"
				scaleHandle.onpointerdown = (event) => {
					if (event.button !== 0) return
					event.stopPropagation()
					const raw = this.world(event.clientX, event.clientY, false)
					const center = { ...this.scaleCenter }
					const vector = { x: raw.x - center.x, y: raw.y - center.y }
					if (Math.hypot(vector.x, vector.y) * this.view.scale < 4) return
					this.drag = { before: copy(this.sketch), groupScale: { center, vector, applied: 1, selected: [...this.selected], removed: [] } }
					this.svg.setPointerCapture?.(event.pointerId)
				}
				scaleHandle.onkeydown = (event) => {
					if (event.key !== "Enter") return
					event.preventDefault()
					event.stopPropagation()
					this.properties.querySelector<HTMLInputElement>('[aria-label="Scale factor"]')?.focus()
				}
				this.node("text", { x: sx + 15, y: sy + 4, fill: "#15803d", "font-size": 12, "pointer-events": "none" }, "Scale")
			}
		}
		if (this.hoverSnap && !this.drag && !this.marquee && !["Select", "Dimension", "Trim", "Split", "Extend"].includes(this.tool)) {
			const snap = this.hoverSnap
			const p = this.screen(snap.position)
			const description = snap.anchor
				? `${snap.anchor.entityId} ${snap.anchor.point}`
				: snap.kind === "origin"
					? "Origin"
					: snap.kind === "intersection"
						? "Intersection"
						: snap.kind === "midpoint"
							? "Midpoint"
							: "Grid"
			this.node("rect", {
				x: p.x - 6,
				y: p.y - 6,
				width: 12,
				height: 12,
				fill: "none",
				stroke: "#16a34a",
				"stroke-width": 2,
				"pointer-events": "none",
				"data-snap-target": snap.kind,
				"aria-label": `Snap to ${description}`
			})
			this.node(
				"text",
				{
					x: p.x + 12,
					y: p.y - 12,
					fill: "#15803d",
					"font-size": 12,
					"paint-order": "stroke",
					stroke: "white",
					"stroke-width": 3,
					"pointer-events": "none",
					"data-snap-label": snap.kind
				},
				description
			)
		}
		this.status.textContent =
			this.result.status === "conflicting"
				? `Conflicting constraints: ${this.result.conflicts.join(", ")}`
				: `${this.result.status === "fully-constrained" ? "Fully constrained" : `Underconstrained · ${this.result.degreesOfFreedom} degrees of freedom`} · ${this.sketch.entities.length} entities · ${this.sketch.relations?.length ?? 0} constraints · Grid ${spacing} mm`
		if (this.result.redundantRelations.length) this.status.textContent += ` · Locally redundant: ${this.result.redundantRelations.join(", ")}`
	}
	private drawRelation(r: SketchRelation) {
		let anchor: Point2D
		try {
			if ("entityId" in r) {
				const e = requireValue(this.sketch.entities.find((e) => e.id === r.entityId))
				anchor = e.type === "arc" ? arcPoint(e, 0.5) : e.type === "circle" ? { x: e.center.x + e.radius, y: e.center.y } : this.anchorPoint(this.defaultAnchor(e))
			} else if (r.type === "circularPattern") anchor = circularPatternCenter(r, this.sketch.entities)
			else if (r.type === "linearPattern") anchor = this.anchorPoint(this.defaultAnchor(requireValue(this.sketch.entities.find((e) => e.id === r.sources[0]))))
			else if (r.type === "offsetChain") anchor = this.anchorPoint({ entityId: requireValue(r.targets[0]), point: "p0" })
			else if (r.type === "fixed") anchor = this.anchorPoint(r.anchor)
			else if (r.type === "radiusDifference") {
				const circle = requireValue(this.sketch.entities.find((e) => e.id === r.a))
				if (circle.type !== "circle") return
				anchor = { x: circle.center.x + circle.radius * Math.cos(Math.PI / 4), y: circle.center.y + circle.radius * Math.sin(Math.PI / 4) }
			} else if (typeof r.a === "string") anchor = this.anchorPoint(this.defaultAnchor(requireValue(this.sketch.entities.find((e) => e.id === r.a))))
			else anchor = this.anchorPoint(r.a)
		} catch {
			return
		}
		const a = this.screen(anchor)
		const index = Math.max(0, this.sketch.relations?.filter((r) => "value" in r).findIndex((item) => item.id === r.id) ?? 0)
		const p = this.screen(r.labelPosition ?? { x: anchor.x + 18, y: anchor.y + 20 + index * 12 })
		let text =
			"value" in r
				? `${r.type === "diameter" ? "Ø" : r.type === "radius" ? "R" : ""}${Number((r.reference ? measureSketchDimension(this.sketch.entities, r) : r.value).toFixed(3))}${r.type === "angle" || r.type === "arcSweep" || r.type === "rotation" ? "°" : ""}`
				: constraintLabel(r.type)
		if (r.type === "chamfer") text += ` × ${Number(r.secondValue.toFixed(3))}${r.mode === "distance-angle" ? "°" : ""}`
		if (r.reference) text = `(${text})`
		const color = this.result.conflicts.includes(r.id)
			? "#dc2626"
			: r.id === this.selectedRelation
				? "#d97706"
				: this.result.redundantRelations.includes(r.id)
					? "#a16207"
					: r.reference
						? "#64748b"
						: "#334155"
		if (r.type === "linearPattern" && r.id === this.selectedRelation) {
			const { columns, rows, rowStep } = patternLayout(r)
			this.node("circle", { cx: a.x, cy: a.y, r: 3, fill: color, "pointer-events": "none", "data-pattern-origin": r.id })
			for (const axis of ["column", "row"] as const) {
				if ((axis === "column" ? columns : rows) <= 1) continue
				const step = axis === "column" ? r.step : rowStep
				const end = this.screen({ x: anchor.x + step.x, y: anchor.y + step.y })
				this.node("line", { x1: a.x, y1: a.y, x2: end.x, y2: end.y, stroke: color, "stroke-dasharray": "4 4", "pointer-events": "none", "data-pattern-step-guide": axis })
				const handle = this.node("circle", {
					cx: end.x,
					cy: end.y,
					r: 7,
					fill: color,
					stroke: "white",
					"stroke-width": 2,
					role: "button",
					"aria-label": `Adjust pattern ${axis} spacing`,
					"data-pattern-step": axis
				})
				handle.style.cursor = "move"
				handle.onpointerdown = (event) => {
					if (event.button !== 0) return
					event.stopPropagation()
					this.drag = { patternStep: { id: r.id, axis }, before: copy(this.sketch) }
					this.svg.setPointerCapture?.(event.pointerId)
				}
				this.node(
					"text",
					{ x: end.x + 12, y: end.y - 12, fill: color, "font-size": 13, "pointer-events": "none", "paint-order": "stroke", stroke: "white", "stroke-width": 3 },
					`${axis === "column" ? "Column" : "Row"} step: ${Number(Math.hypot(step.x, step.y).toFixed(1))} mm`
				)
			}
		}
		if (r.type === "circularPattern" && r.id === this.selectedRelation) {
			const source = requireValue(this.sketch.entities.find((e) => e.id === r.sources[0]))
			const sourcePoint = this.anchorPoint(this.defaultAnchor(source))
			const startAngle = Math.atan2(sourcePoint.y - anchor.y, sourcePoint.x - anchor.x)
			const radius = Math.max(40, Math.hypot(sourcePoint.x - anchor.x, sourcePoint.y - anchor.y) * this.view.scale + 18)
			const sweep = (r.angle * Math.PI) / 180
			const steps = Math.max(2, Math.ceil(Math.abs(r.angle) / 8))
			const guide = Array.from({ length: steps + 1 }, (_, i) => ({
				x: a.x + radius * Math.cos(startAngle + (sweep * i) / steps),
				y: a.y - radius * Math.sin(startAngle + (sweep * i) / steps)
			}))
			this.node("path", {
				d: `M ${guide.map((p) => `${p.x} ${p.y}`).join(" L ")}`,
				fill: "none",
				stroke: color,
				"stroke-dasharray": "4 4",
				"pointer-events": "none",
				"data-pattern-angle-guide": r.id
			})
			const end = requireValue(guide[guide.length - 1])
			const angleHandle = this.node("circle", {
				cx: end.x,
				cy: end.y,
				r: 7,
				fill: color,
				stroke: "white",
				"stroke-width": 2,
				role: "button",
				"aria-label": "Adjust circular pattern angle",
				"data-pattern-angle": r.id
			})
			angleHandle.style.cursor = "crosshair"
			angleHandle.onpointerdown = (event) => {
				if (event.button !== 0) return
				event.stopPropagation()
				const point = this.world(event.clientX, event.clientY, false)
				this.drag = { patternAngle: { id: r.id, last: Math.atan2(point.y - anchor.y, point.x - anchor.x), value: r.angle }, before: copy(this.sketch) }
				this.svg.setPointerCapture?.(event.pointerId)
			}
			this.node(
				"text",
				{ x: end.x + 12, y: end.y - 12, fill: color, "font-size": 13, "pointer-events": "none", "paint-order": "stroke", stroke: "white", "stroke-width": 3 },
				`${Number(r.angle.toFixed(1))}°`
			)
			const handle = this.node("circle", {
				cx: a.x,
				cy: a.y,
				r: 6,
				fill: "white",
				stroke: color,
				"stroke-width": 2,
				role: "button",
				"aria-label": "Move circular pattern center",
				"data-pattern-center": r.id
			})
			handle.style.cursor = "move"
			handle.onpointerdown = (event) => {
				if (event.button !== 0) return
				event.stopPropagation()
				this.drag = r.centerAnchor ? { anchor: { ...r.centerAnchor }, before: copy(this.sketch) } : { patternCenter: r.id, before: copy(this.sketch) }
				this.svg.setPointerCapture?.(event.pointerId)
			}
		}
		this.node("line", { x1: a.x, y1: a.y, x2: p.x, y2: p.y, stroke: color, "stroke-width": 1 })
		const label = this.node(
			"text",
			{ x: p.x, y: p.y, fill: color, "font-size": 13, "paint-order": "stroke", stroke: "white", "stroke-width": 4, role: "button", "aria-label": `Constraint ${r.id}: ${text}` },
			text
		)
		label.style.cursor = "pointer"
		label.setAttribute("tabindex", "0")
		label.ondblclick = (event) => {
			event.stopPropagation()
			this.editDimensionLabel(r.id, p)
		}
		label.onkeydown = (event) => {
			if (event.key !== "Enter") return
			event.preventDefault()
			event.stopPropagation()
			this.selectedRelation = r.id
			this.renderProperties()
			this.editDimensionLabel(r.id, p)
		}

		label.onpointerdown = (e) => {
			e.stopPropagation()
			this.selectedRelation = r.id
			this.renderProperties()
			this.drag = { label: r.id, before: copy(this.sketch) }
			label.setPointerCapture?.(e.pointerId)
		}
	}
	private editDimensionLabel(id: string, position: Point2D) {
		const relation = this.sketch.relations?.find((r) => r.id === id)
		if (this.disposed || !relation || !("value" in relation) || relation.reference) return
		this.svg.querySelector("[data-dimension-editor]")?.remove()
		this.drag = null
		const width = Math.min(320, Math.max(1, this.width - 16))
		const height = Math.min(160, Math.max(1, this.height - 16))
		const overlay = this.node("foreignObject", {
			x: Math.max(0, Math.min(position.x, this.width - width - 8)),
			y: Math.max(0, Math.min(position.y - 24, this.height - height - 8)),
			width,
			height,
			"data-dimension-editor": id
		})
		const panel = document.createElement("div")
		panel.style.cssText = "height:100%;box-sizing:border-box;overflow:auto;background:white;border:1px solid #94a3b8;border-radius:6px;padding:8px;font:13px sans-serif;color:#334155"
		panel.onpointerdown = (e) => e.stopPropagation()
		panel.ondblclick = (e) => e.stopPropagation()
		const field = document.createElement("input")
		field.type = "text"
		field.inputMode = "text"
		field.value = relation.expression ?? String(relation.value)
		field.setAttribute("aria-label", "Edit dimension value")
		field.style.cssText = "width:100%;box-sizing:border-box"
		const units = document.createElement("label")
		units.textContent = ["angle", "arcSweep", "rotation"].includes(relation.type) ? "Dimension (degrees)" : "Dimension (mm)"
		units.append(field)
		const error = document.createElement("div")
		error.setAttribute("role", "alert")
		error.style.cssText = "color:#b91c1c;font-size:12px;overflow-wrap:anywhere;margin-top:6px"
		const close = () => {
			overlay.remove()
			this.svg.focus()
		}
		const commit = () => {
			if (this.disposed || !overlay.isConnected) return
			try {
				const value = this.parseValue(field.value, ["angle", "arcSweep", "rotation"].includes(relation.type) ? "angle" : "length")
				const relations = normalizeSketchRelations(
					(this.sketch.relations ?? []).map((r) =>
						r.id === id
							? { ...r, value, ...(isMeasuredDimension(r) ? { expression: field.value.trim() === String(value) ? undefined : field.value.trim() } : {}) }
							: r
					),
					this.sketch.variables
				)
				const result = solveSketch(this.sketch.entities, relations ?? [])
				if (result.status === "conflicting") throw Error("Value conflicts with existing constraints.")
				close()
				if (value !== relation.value || (isMeasuredDimension(relation) && field.value.trim() !== relation.expression))
					this.edit(() => {
						this.sketch.relations = relations
					})
			} catch (reason) {
				error.textContent = reason instanceof Error ? reason.message : String(reason)
				field.setAttribute("aria-invalid", "true")
				field.focus()
			}
		}
		panel.onkeydown = (e) => {
			e.stopPropagation()
			if (e.key === "Enter" && e.target === field) {
				e.preventDefault()
				commit()
			}
			if (e.key === "Escape") {
				e.preventDefault()
				close()
			}
		}
		panel.append(units)
		const actions = document.createElement("div")
		actions.style.cssText = "display:flex;gap:6px;margin-top:6px"
		for (const button of [this.button(actions, "Apply dimension", commit), this.button(actions, "Cancel dimension", close)])
			button.style.cssText += ";flex:1;min-width:0;white-space:normal"
		panel.append(actions)
		panel.append(error)
		overlay.append(panel)
		field.focus()
		field.select()
	}

	private finishSketch() {
		if (this.disposed) return
		if (this.splinePoints.length) {
			this.status.textContent = "Finish the spline or press Escape before finishing the sketch."
			return
		}
		try {
			const result = solveSketch(this.sketch.entities, this.sketch.relations ?? [])
			if (result.status === "conflicting") throw Error("Resolve conflicting constraints before finishing the sketch.")
			const next = materializeSketch({ ...this.sketch, entities: result.entities })
			this.finish(next)
			this.dispose()
		} catch (error) {
			this.status.textContent = error instanceof Error ? error.message : String(error)
		}
	}
	/** Dock sketch tools and canvas into the existing part layout. */
	mount(viewport: HTMLElement, propertiesHost: HTMLElement) {
		this.root.style.position = "absolute"
		this.root.style.inset = "0"
		this.root.style.zIndex = "10"
		this.root.style.gridTemplateColumns = "minmax(0,1fr)"
		this.properties.setAttribute("data-sketch-workspace", "")
		this.properties.setAttribute("aria-label", "Sketch properties")
		this.properties.style.cssText = "min-width:0;height:100%;min-height:0;overflow:auto;contain:size"
		this.properties.addEventListener("keydown", (event) => this.key(event))
		propertiesHost.replaceChildren(this.properties)
		viewport.append(this.root)
		this.fit()
	}
	focus() {
		if (!this.disposed) this.svg.focus()
	}
	dispose() {
		if (this.disposed) return
		this.disposed = true
		this.drag = null
		this.marquee = null
		this.observer?.disconnect()
		if (this.resizeFrame !== undefined) cancelAnimationFrame(this.resizeFrame)
		this.properties.remove()
		this.root.remove()
	}
}
