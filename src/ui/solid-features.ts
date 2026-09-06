import type { PartDocument } from "../schema"
import type { Point2D } from "../types"
import { PartBuilder, circle, rectangle, v2 } from "../sdk"
import type { SolidStep } from "../solid-model"
import { extrusionFor, sketchFor, stepLoops, replaceStepLoops, validateSolidEdit } from "../solid-edit"
import { extrudeSolidFeature, getExtrudedFaceDescriptors } from "../cad/extrude"
import { requireValue } from "../required"
import { button, field, note, numberField, panelStyle, section, select, vector, validateFields } from "./model-fields"

/** Transactional solid editor: drafts never modify the accepted document until Apply succeeds. */
export class SolidFeaturePanel {
	readonly root = document.createElement("aside")
	private draft: PartDocument
	private loops = new Map<string, Point2D[][]>()
	private selected = 0
	private status = document.createElement("p")
	constructor(
		private accepted: PartDocument,
		private readonly apply: (next: PartDocument) => void
	) {
		this.draft = structuredClone(accepted)
		this.root.style.cssText = panelStyle
		this.root.setAttribute("aria-label", "Solid feature editor")
		this.render()
	}
	private changed = () => {
		this.status.textContent = "Unsaved changes — Apply to rebuild the part."
	}
	private attempt(action: () => void) {
		try {
			action()
		} catch (error) {
			this.status.textContent = error instanceof Error ? error.message : String(error)
			this.status.setAttribute("role", "alert")
		}
	}
	private render() {
		this.root.replaceChildren()
		const title = document.createElement("h2")
		title.textContent = "Solid features"
		title.style.margin = "0 0 8px"
		this.root.append(title)
		note(this.root, "Edit dimensions in mm. Select a feature, change its profile or operation, then Apply. Orbit and zoom the preview to inspect the result.")
		const actions = document.createElement("div")
		actions.style.cssText = "display:flex;gap:6px;flex-wrap:wrap"
		actions.append(
			button("Apply changes", () =>
				this.attempt(() => {
					validateFields(this.root)
					const candidate = structuredClone(this.draft)
					for (const step of candidate.solidSteps ?? []) {
						const loops = this.loops.get(step.id)
						if (loops) replaceStepLoops(candidate, step, loops)
					}
					candidate.cad = undefined
					candidate.tree = undefined
					validateSolidEdit(candidate)
					this.apply(candidate)
					this.accepted = structuredClone(candidate)
					this.draft = structuredClone(candidate)
					this.loops.clear()
					this.render()
					this.status.textContent = "Changes applied."
				})
			),
			button("Discard changes", () => {
				this.draft = structuredClone(this.accepted)
				this.loops.clear()
				this.render()
			})
		)
		this.root.append(actions)
		this.status = document.createElement("p")
		this.status.setAttribute("role", "status")
		this.status.style.cssText = "font-size:13px;color:#a04115"
		this.root.append(this.status)
		this.draft.solidSteps ??= []
		const steps = this.draft.solidSteps
		const list = document.createElement("div")
		list.style.cssText = "display:flex;flex-direction:column;gap:4px;margin:12px 0;max-height:220px;overflow:auto"
		this.root.append(list)
		steps.forEach((step, i) => {
			const entry = button(`${i + 1}. ${step.id} · ${step.operation}`, () => {
				this.selected = i
				this.render()
			})
			entry.setAttribute("aria-pressed", String(i === this.selected))
			if (i === this.selected) entry.style.background = "#dcecf9"
			entry.style.textAlign = "left"
			list.append(entry)
		})
		const add = document.createElement("div")
		add.style.cssText = "display:flex;gap:6px"
		this.root.append(add)
		for (const type of ["extrusion", "revolve"] as const)
			add.append(
				button(`Add ${type}`, () =>
					this.attempt(() => {
						const builder = new PartBuilder()
						let id = `${type}-${steps.length + 1}`
						while (steps.some((s) => s.id === id) || this.draft.features.some((f) => f.id === id || f.id === `${id}/sketch`)) id += "-new"
						if (type === "revolve") builder.revolve(id, { outline: [v2(0, 0), v2(10, 0), v2(10, 10), v2(0, 10)] })
						else builder.extrude(id, { outline: rectangle(v2(0, 0), 20, 20), depth: 10 })
						this.draft.features.push(...builder.document.features)
						steps.push(...requireValue(builder.document.solidSteps))
						this.selected = steps.length - 1
						this.render()
						this.changed()
					})
				)
			)
		const step = steps[this.selected]
		if (!step) return
		const props = section(this.root, `Edit ${step.id}`, true)
		field(props, "Feature name", step.id, (value) => {
			if (!value.trim() || steps.some((s) => s !== step && s.id === value)) {
				this.status.textContent = "Feature names must be unique and nonempty."
				return
			}
			const loops = this.loops.get(step.id)
			this.loops.delete(step.id)
			step.id = value
			if (loops) this.loops.set(value, loops)
			this.changed()
		})
		select(props, "Operation", step.operation, ["join", "cut", "intersect"], (v) => {
			step.operation = v as SolidStep["operation"]
			this.changed()
		})
		if (step.type === "revolve") {
			numberField(props, "Revolve angle (degrees)", step.angle ?? 360, (n) => {
				step.angle = n
				this.changed()
			})
			numberField(props, "Revolve segments", step.segments ?? 96, (n) => {
				step.segments = n
				this.changed()
			})
		} else {
			const feature = extrusionFor(this.draft, step)
			numberField(props, "Extrusion depth (mm)", feature.depth, (n) => {
				feature.depth = n
				this.changed()
			})
			numberField(props, "Top scale", step.topScale ?? 1, (n) => {
				step.topScale = n
				this.changed()
			})
			const sketch = sketchFor(this.draft, step)
			const choices = [
				{ value: "XY", label: "XY plane" },
				{ value: "XZ", label: "XZ plane" },
				{ value: "YZ", label: "YZ plane" }
			]
			for (const other of steps.slice(0, this.selected)) {
				if (other.type !== "extrusion") continue
				try {
					for (const face of getExtrudedFaceDescriptors(extrudeSolidFeature(this.draft, extrusionFor(this.draft, other))))
						choices.push({ value: `${other.featureId}|${face.faceId}`, label: `${other.id}: ${face.label}` })
				} catch {}
			}
			select(props, "Sketch support", sketch.target.type === "plane" ? sketch.target.plane : `${sketch.target.face.extrudeId}|${sketch.target.face.faceId}`, choices, (v) => {
				const [extrudeId, faceId] = v.split("|")
				sketch.target = faceId ? { type: "face", face: { type: "extrudeFace", extrudeId: requireValue(extrudeId), faceId } } : { type: "plane", plane: v as "XY" | "XZ" | "YZ" }
				this.changed()
			})
		}
		step.translation ??= { x: 0, y: 0, z: 0 }
		vector(props, "Translation (mm)", step.translation, this.changed)
		const order = document.createElement("div")
		order.style.cssText = "display:flex;gap:6px;flex-wrap:wrap"
		props.append(order)
		for (const [label, delta] of [
			["Move up", -1],
			["Move down", 1]
		] as const) {
			const b = button(label, () => {
				const i = this.selected
				const j = i + delta
				;[steps[i], steps[j]] = [requireValue(steps[j]), requireValue(steps[i])]
				this.selected = j
				this.render()
				this.changed()
			})
			b.disabled = this.selected + delta < 0 || this.selected + delta >= steps.length
			order.append(b)
		}
		order.append(
			button("Delete feature", () => {
				if (step.type === "extrusion") {
					const feature = extrusionFor(this.draft, step)
					const sketch = sketchFor(this.draft, step)
					if (!steps.some((s) => s !== step && s.featureId === step.featureId)) {
						this.draft.features = this.draft.features.filter(
							(f) => f.id !== feature.id && f.id !== sketch.id && !(f.type === "chamfer" && f.target.edge.extrudeId === feature.id)
						)
					}
				}
				steps.splice(this.selected, 1)
				this.loops.delete(step.id)
				this.selected = Math.max(0, this.selected - 1)
				this.render()
				this.changed()
			})
		)
		this.attempt(() => this.renderProfiles(step))
		this.renderFinishes(step)
	}
	private renderProfiles(step: SolidStep) {
		const loops = this.loops.get(step.id) ?? stepLoops(this.draft, step)
		this.loops.set(step.id, loops)
		const profiles = section(this.root, step.type === "revolve" ? "Revolve profile (radius / height)" : "Sketch profile and holes", true)
		loops.forEach((points, index) => {
			const group = section(profiles, index === 0 ? "Outline" : `Hole ${index}`, index === 0)
			const minX = Math.min(...points.map((p) => p.x))
			const maxX = Math.max(...points.map((p) => p.x))
			const minY = Math.min(...points.map((p) => p.y))
			const maxY = Math.max(...points.map((p) => p.y))
			const cx = (minX + maxX) / 2
			const cy = (minY + maxY) / 2
			const width = maxX - minX
			const height = maxY - minY
			numberField(group, "Profile center X", cx, (n) => {
				for (const p of points) p.x += n - cx
				this.render()
				this.changed()
			})
			numberField(group, "Profile center Y", cy, (n) => {
				for (const p of points) p.y += n - cy
				this.render()
				this.changed()
			})
			numberField(group, "Profile width (mm)", width, (n) => {
				if (n <= 0) return
				for (const p of points) p.x = step.type === "revolve" ? minX + ((p.x - minX) * n) / width : cx + ((p.x - cx) * n) / width
				this.render()
				this.changed()
			})
			numberField(group, "Profile height (mm)", height, (n) => {
				if (n <= 0) return
				for (const p of points) p.y = step.type === "revolve" ? minY + ((p.y - minY) * n) / height : cy + ((p.y - cy) * n) / height
				this.render()
				this.changed()
			})
			this.profileSvg(group, points)
			const shape = section(group, "Replace profile shape")
			let radius = 10
			let segments = 64
			let shapeWidth = 20
			let shapeHeight = 20
			numberField(shape, "Circle radius (mm)", radius, (n) => {
				radius = n
			})
			numberField(shape, "Circle segments", segments, (n) => {
				segments = n
			})
			shape.append(
				button("Use circle", () =>
					this.attempt(() => {
						if (radius <= 0 || !Number.isInteger(segments) || segments < 8 || segments > 512) throw Error("Use a positive radius and 8–512 circle segments.")
						loops[index] = circle(v2(cx, cy), radius, segments)
						this.render()
						this.changed()
					})
				)
			)
			numberField(shape, "Rectangle width (mm)", shapeWidth, (n) => {
				shapeWidth = n
			})
			numberField(shape, "Rectangle height (mm)", shapeHeight, (n) => {
				shapeHeight = n
			})
			shape.append(
				button("Use rectangle", () => {
					if (shapeWidth <= 0 || shapeHeight <= 0) return
					loops[index] = rectangle(v2(cx, cy), shapeWidth, shapeHeight)
					this.render()
					this.changed()
				})
			)
			const vertices = section(group, `Vertices (${points.length}) — exact coordinates`)
			points.forEach((p, i) => {
				const row = section(vertices, `Vertex ${i}`)
				numberField(row, `Vertex ${i} X`, p.x, (n) => {
					p.x = n
					this.changed()
				})
				numberField(row, `Vertex ${i} Y`, p.y, (n) => {
					p.y = n
					this.changed()
				})
				row.append(
					button("Insert point after", () => {
						const q = requireValue(points[(i + 1) % points.length])
						points.splice(i + 1, 0, { x: (p.x + q.x) / 2, y: (p.y + q.y) / 2 })
						this.render()
						this.changed()
					}),
					button("Remove point", () => {
						if (points.length <= 3) {
							this.status.textContent = "A profile needs at least three vertices."
							return
						}
						points.splice(i, 1)
						this.render()
						this.changed()
					})
				)
			})
			if (index > 0)
				group.append(
					button("Remove hole", () => {
						loops.splice(index, 1)
						this.render()
						this.changed()
					})
				)
		})
		if (step.type === "extrusion")
			profiles.append(
				button("Add circular hole", () => {
					loops.push(circle(v2(0, 0), 2, 32))
					this.render()
					this.changed()
				})
			)
	}
	private profileSvg(parent: HTMLElement, points: Point2D[]) {
		const ns = "http://www.w3.org/2000/svg"
		const svg = document.createElementNS(ns, "svg")
		svg.setAttribute("viewBox", "0 0 300 190")
		svg.style.cssText = "width:100%;height:190px;background:#eaf0f7;touch-action:none;border:1px solid #cbd5e1"
		svg.setAttribute("aria-label", "Profile preview; drag a vertex to edit, or use exact coordinate fields below.")
		const minX = Math.min(...points.map((p) => p.x))
		const maxX = Math.max(...points.map((p) => p.x))
		const minY = Math.min(...points.map((p) => p.y))
		const maxY = Math.max(...points.map((p) => p.y))
		const scale = Math.min(260 / Math.max(1, maxX - minX), 150 / Math.max(1, maxY - minY))
		const cx = (minX + maxX) / 2
		const cy = (minY + maxY) / 2
		const polygon = document.createElementNS(ns, "polygon")
		polygon.setAttribute("fill", "#83c8b4")
		polygon.setAttribute("stroke", "#257965")
		svg.append(polygon)
		const draw = () => polygon.setAttribute("points", points.map((p) => `${150 + (p.x - cx) * scale},${95 - (p.y - cy) * scale}`).join(" "))
		draw()
		points.forEach((p, i) => {
			const node = document.createElementNS(ns, "circle")
			node.setAttribute("cx", String(150 + (p.x - cx) * scale))
			node.setAttribute("cy", String(95 - (p.y - cy) * scale))
			node.setAttribute("r", points.length > 50 ? "2" : "4")
			node.setAttribute("fill", "#234d74")
			node.style.cursor = "move"
			const title = document.createElementNS(ns, "title")
			title.textContent = `Vertex ${i}: ${p.x}, ${p.y}`
			node.append(title)
			svg.append(node)
			let dragging = false
			node.onpointerdown = (e) => {
				dragging = true
				node.setPointerCapture(e.pointerId)
			}
			node.onpointermove = (e) => {
				if (!dragging) return
				const rect = svg.getBoundingClientRect()
				p.x = cx + (((e.clientX - rect.left) * 300) / rect.width - 150) / scale
				p.y = cy - (((e.clientY - rect.top) * 190) / rect.height - 95) / scale
				node.setAttribute("cx", String(150 + (p.x - cx) * scale))
				node.setAttribute("cy", String(95 - (p.y - cy) * scale))
				draw()
				this.changed()
			}
			node.onpointerup = () => {
				if (dragging) {
					dragging = false
					this.render()
					this.changed()
				}
			}
			node.onpointercancel = () => {
				dragging = false
			}
		})
		parent.append(svg)
	}
	private renderFinishes(step: SolidStep) {
		const group = section(this.root, "Fillets and chamfers", true)
		for (const [i, finish] of (step.finishes ?? []).entries()) {
			const row = section(group, `Profile ${finish.kind} ${i + 1}`, true)
			select(row, "Finish kind", finish.kind, ["fillet", "chamfer"], (v) => {
				finish.kind = v as "fillet" | "chamfer"
				this.changed()
			})
			numberField(row, "Radius / distance (mm)", finish.radius, (n) => {
				finish.radius = n
				this.changed()
			})
			numberField(row, "Arc segments", finish.segments ?? 8, (n) => {
				finish.segments = n
				this.changed()
			})
			field(row, "Vertex indices (blank = all)", finish.vertices?.join(", ") ?? "", (v) => {
				finish.vertices = v.trim() ? v.split(",").map(Number) : undefined
				this.changed()
			})
			row.append(
				button("Remove finish", () => {
					step.finishes?.splice(i, 1)
					this.render()
					this.changed()
				})
			)
		}
		group.append(
			button("Add profile fillet", () => {
				step.finishes ??= []
				step.finishes.push({ kind: "fillet", radius: 1, vertices: [0], segments: 8 })
				this.render()
				this.changed()
			})
		)
		if (step.type === "extrusion") {
			let edges: string[] = []
			try {
				edges = extrudeSolidFeature(this.draft, extrusionFor(this.draft, step)).solid.edges.map((e) => e.id)
			} catch {}
			for (const [i, finish] of (step.edgeFinishes ?? []).entries()) {
				const row = section(group, `Edge ${finish.kind} ${i + 1}`, true)
				select(row, "Source edge", finish.edgeId, edges, (v) => {
					finish.edgeId = v
					this.changed()
				})
				select(row, "Edge finish kind", finish.kind, ["fillet", "chamfer"], (v) => {
					finish.kind = v as "fillet" | "chamfer"
					this.changed()
				})
				numberField(row, "Edge radius / distance (mm)", finish.radius, (n) => {
					finish.radius = n
					this.changed()
				})
				numberField(row, "Second chamfer distance (mm)", finish.distance2 ?? finish.radius, (n) => {
					finish.distance2 = n
					this.changed()
				})
				numberField(row, "Edge arc segments", finish.segments ?? 8, (n) => {
					finish.segments = n
					this.changed()
				})
				row.append(
					button("Remove edge finish", () => {
						step.edgeFinishes?.splice(i, 1)
						this.render()
						this.changed()
					})
				)
			}
			group.append(
				button("Add edge fillet", () => {
					if (!edges[0]) return
					step.edgeFinishes ??= []
					step.edgeFinishes.push({ kind: "fillet", edgeId: edges[0], radius: 1, segments: 8 })
					this.render()
					this.changed()
				})
			)
			note(group, "Edge finishes support convex straight source edges. Invalid or overlapping finishes are rejected on Apply.")
		}
	}
}
