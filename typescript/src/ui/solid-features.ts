import { sketchSupportEdges } from "../sketch-context"
import { materializeSketch } from "../cad/sketch"
import type { Sketch, SketchPlane, SketchTarget } from "../schema"
import { SketchWorkspace } from "./sketch-workspace"
import { capsule as capsulePrimitive, circle as circlePrimitive, rectangle as rectanglePrimitive, primitivePoints, normalizePrimitive } from "../sketch-primitives"
import type { SketchPrimitive } from "../schema"
import { navigationKey, readableName, type ModelNavigationNode } from "./model-navigation"
import type { SketchSource } from "./solid-source"
import type { PartDocument } from "../schema"
import type { Point2D } from "../types"
import { PartBuilder } from "../sdk"
import { circle, rectangle, v2 } from "../model-dsl"
import type { SolidStep } from "../solid-model"
import { extrusionFor, sketchFor, stepLoops, replaceStepLoops, replaceStepOutline, primitiveForLoop, validateSolidEdit } from "../solid-edit"
import { extrudeSolidFeature, getExtrudedFaceDescriptors } from "../cad/extrude"
import { requireValue } from "../required"
import { button, field, note, numberField, panelStyle, section, select, vector, validateFields } from "./model-fields"

/** Transactional solid editor: drafts never modify the accepted document until Apply succeeds. */
export class SolidFeaturePanel {
	readonly root = document.createElement("aside")
	private workspace: SketchWorkspace | null = null
	private sketchViewport?: HTMLElement
	public useSketchViewport(viewport: HTMLElement): void {
		this.sketchViewport = viewport
	}
	private disposed = false
	public dispose(): void {
		if (this.disposed) return
		this.disposed = true
		this.workspace?.dispose()
		this.workspace = null
		this.root.remove()
	}
	private draft: PartDocument
	private loops = new Map<string, Point2D[][]>()
	private selected = 0
	private navigation?: { select: (key: string) => void; change: () => void }
	private navigationSelection: (string | number)[] = ["part"]
	public useProjectNavigation(select: (key: string) => void, change: () => void): void {
		this.navigation = { select, change }
		this.root.style.cssText = "height:100%;overflow:auto;padding:12px;box-sizing:border-box;min-width:0;background:#f8fafc"
		this.render()
	}
	public getNavigation(): ModelNavigationNode[] {
		return [
			...this.draft.features
				.filter((f): f is Sketch => f.type === "sketch")
				.map((sketch) => ({ key: navigationKey("standalone-sketch", sketch.id), label: sketch.name ?? sketch.id, children: [] })),
			...(this.draft.solidSteps ?? []).map((step) => {
				const loops = this.loops.get(step.id) ?? stepLoops(this.draft, step)
				if (step.type === "loft" || step.type === "shell") return { key: navigationKey("feature", step.id), label: readableName(step.id), children: [] }
				return {
					key: navigationKey("feature", step.id),
					label: readableName(step.id),
					children: [
						{
							key: navigationKey("sketch", step.id),
							label: step.type === "revolve" ? "Revolve profile" : "Sketch",
							children: loops.map((points, loop) => ({
								key: navigationKey("profile", step.id, loop),
								label: loop === 0 ? "Outline" : `Hole ${loop}`,
								children: primitiveForLoop(this.draft, step, points)
									? [
											{
												key: navigationKey("entity", step.id, loop, 0),
												label: readableName(requireValue(primitiveForLoop(this.draft, step, points)).type)
											}
										]
									: points.map((_, edge) => ({ key: navigationKey("entity", step.id, loop, edge), label: `Line ${edge + 1}` }))
							}))
						}
					]
				}
			})
		]
	}
	public selectNavigation(key: string): void {
		const target = JSON.parse(key) as (string | number)[]
		const index = this.draft.solidSteps?.findIndex((step) => step.id === target[1]) ?? -1
		if (index >= 0) this.selected = index
		this.navigationSelection = target
		this.source = null
		this.render()
	}

	private source: SketchSource | null = null
	public clearSelection(): void {
		this.selectNavigation(navigationKey("part"))
		this.navigation?.select(navigationKey("part"))
	}
	public selectSource(source: SketchSource): void {
		const index = this.draft.solidSteps?.findIndex((step) => step.id === source.stepId) ?? -1
		if (index < 0) return
		this.selected = index
		this.source = source
		const step = requireValue(this.draft.solidSteps?.[index])
		const loop = stepLoops(this.draft, step)[source.loopIndex]
		this.navigationSelection = ["entity", source.stepId, source.loopIndex, loop && primitiveForLoop(this.draft, step, loop) ? 0 : source.edgeIndex]
		this.navigation?.select(navigationKey(...this.navigationSelection))
		this.render()
		this.root.querySelector("[data-selected-source]")?.scrollIntoView?.({ block: "nearest" })
	}
	private status = document.createElement("p")
	private dirty = false
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
		this.dirty = true
		this.status.textContent = "Unsaved changes — Apply to rebuild the part."
		this.navigation?.change()
	}
	private attempt(action: () => void) {
		if (this.disposed) return
		try {
			action()
		} catch (error) {
			this.status.textContent = error instanceof Error ? error.message : String(error)
			this.status.setAttribute("role", "alert")
		}
	}
	private render() {
		if (this.disposed || this.workspace) return
		this.root.replaceChildren()
		const title = document.createElement("h2")
		title.textContent = this.navigation ? "Properties" : "Solid features"
		title.style.margin = "0 0 8px"
		this.root.append(title)
		note(this.root, "Edit dimensions in mm. Select a feature, change its profile or operation, then Apply. Orbit and zoom the preview to inspect the result.")
		const actions = document.createElement("div")
		actions.style.cssText = "display:flex;gap:6px;flex-wrap:wrap"
		actions.append(
			button("Apply changes", () =>
				this.attempt(() => {
					if (this.workspace) throw Error("Finish or cancel the sketch before applying part changes.")
					validateFields(this.root)
					const candidate = structuredClone(this.draft)
					for (const step of candidate.solidSteps ?? []) {
						const loops = this.loops.get(step.id)
						if (loops) replaceStepLoops(candidate, step, loops)
					}
					candidate.cad = undefined
					candidate.tree = undefined
					if (candidate.solidSteps?.length) validateSolidEdit(candidate)
					else for (const feature of candidate.features) if (feature.type === "sketch") materializeSketch(feature)
					this.apply(candidate)
					this.dirty = false
					this.accepted = structuredClone(candidate)
					this.draft = structuredClone(candidate)
					this.loops.clear()
					this.render()
					this.status.textContent = "Changes applied."
				})
			),
			button("Discard changes", () => {
				this.workspace?.dispose()
				this.workspace = null
				this.dirty = false
				this.draft = structuredClone(this.accepted)
				this.loops.clear()
				this.render()
			})
		)
		this.root.append(actions)
		if (this.navigation) actions.style.cssText += ";position:sticky;top:0;z-index:2;background:#f8fafc;padding:8px 0"
		this.status = document.createElement("p")
		this.status.setAttribute("role", "status")
		if (this.dirty) this.status.textContent = "Unsaved changes — Apply or Discard."
		this.status.style.cssText = "font-size:13px;color:#a04115"
		this.root.append(this.status)
		this.draft.solidSteps ??= []
		const steps = this.draft.solidSteps
		const list = document.createElement("div")
		list.style.cssText = "display:flex;flex-direction:column;gap:4px;margin:12px 0;max-height:220px;overflow:auto"
		if (!this.navigation) this.root.append(list)
		steps.forEach((step, i) => {
			const entry = button(`${i + 1}. ${step.id} · ${step.operation}`, () => {
				this.selected = i
				this.source = null
				this.render()
			})
			entry.setAttribute("aria-pressed", String(i === this.selected))
			if (i === this.selected) entry.style.background = "#dcecf9"
			entry.style.textAlign = "left"
			list.append(entry)
		})
		const add = document.createElement("div")
		add.style.cssText = "display:flex;gap:6px;flex-wrap:wrap"
		this.root.append(add)
		if (this.navigation) add.hidden = this.navigationSelection[0] !== "part"
		let target: SketchTarget = { type: "plane", plane: "XY" }
		const supports = new Map<string, SketchTarget>(["XY", "XZ", "YZ"].map((plane) => [plane, { type: "plane", plane: plane as SketchPlane }]))
		const supportChoices = ["XY", "XZ", "YZ"].map((plane) => ({ value: plane, label: plane }))
		for (const step of add.hidden ? [] : steps) {
			if (step.type !== "extrusion") continue
			try {
				const feature = extrusionFor(this.draft, step)
				const faces = getExtrudedFaceDescriptors(extrudeSolidFeature(this.draft, feature))
				faces.sort((a, b) => Number(a.label.startsWith("Side")) - Number(b.label.startsWith("Side")))
				for (const face of faces) {
					const value = `face-${supports.size}`
					supports.set(value, { type: "face", face: { type: "extrudeFace", extrudeId: feature.id, faceId: face.faceId } })
					supportChoices.push({ value, label: `${step.id}: ${face.label}` })
				}
			} catch {
				// Invalid source features cannot supply a usable sketch support.
			}
		}
		select(add, "Sketch plane", "XY", supportChoices, (value) => {
			target = structuredClone(requireValue(supports.get(value)))
		})
		add.append(
			button("New sketch", () => {
				let id = "Sketch 1"
				for (let n = 2; this.draft.features.some((f) => f.id === id); n++) id = `Sketch ${n}`
				this.openSketch({
					type: "sketch",
					id,
					name: id,
					dirty: false,
					target: structuredClone(target),
					entities: [],
					relations: [],
					dimensions: [],
					vertices: [],
					loops: [],
					profiles: []
				})
			})
		)
		for (const type of ["extrusion", "revolve"] as const)
			add.append(
				button(`Add ${type}`, () =>
					this.attempt(() => {
						const builder = new PartBuilder()
						let id = `${type}-${steps.length + 1}`
						while (steps.some((s) => s.id === id) || this.draft.features.some((f) => f.id === id || f.id === `${id}/sketch`)) id += "-new"
						if (type === "revolve") builder.revolve(id, { outline: [v2(0, 0), v2(10, 0), v2(10, 10), v2(0, 10)] })
						else builder.extrude(id, { outline: rectanglePrimitive(v2(0, 0), 20, 20), depth: 10 })
						this.draft.features.push(...builder.document.features)
						steps.push(...requireValue(builder.document.solidSteps))
						this.selected = steps.length - 1
						if (this.navigation) {
							this.navigationSelection = ["feature", id]
							this.navigation.select(navigationKey(...this.navigationSelection))
						}
						this.render()
						this.changed()
					})
				)
			)
		for (const type of ["loft", "shell"] as const)
			add.append(
				button(`Add ${type}`, () =>
					this.attempt(() => {
						let id = `${type}-${steps.length + 1}`
						while (steps.some((s) => s.id === id)) id += "-new"
						const source = steps.findLast((s) => s.type === "loft")
						if (type === "shell" && !source) throw Error("Create a circular loft before adding a shell.")
						steps.push(
							type === "loft"
								? { id, type, operation: "join", loft: { bottomRadius: 10, topRadius: 15, height: 20, segments: 96 } }
								: { id, type, operation: "cut", sourceId: source?.id, thickness: 1 }
						)
						this.selected = steps.length - 1
						this.navigationSelection = ["feature", id]
						this.render()
						this.changed()
						this.navigation?.select(navigationKey("feature", id))
					})
				)
			)
		const sketches = this.draft.features.filter((f): f is Sketch => f.type === "sketch")
		if (!this.navigation) for (const sketch of sketches) this.root.append(button(`Edit ${sketch.name ?? sketch.id}`, () => this.openSketch(sketch)))
		if (this.navigationSelection[0] === "standalone-sketch") {
			const sketch = requireValue(sketches.find((s) => s.id === this.navigationSelection[1]))
			this.root.append(button("Edit sketch", () => this.openSketch(sketch)))
			this.renderSketchExtrusion(sketch)
			return
		}
		if (!steps.length && sketches.length) this.renderSketchExtrusion(requireValue(sketches.at(-1)))
		const step = steps[this.selected]
		if (!step) return
		if (this.navigation) {
			const kind = this.navigationSelection[0]
			const parts = [readableName(step.id)]
			if (kind === "sketch" || kind === "profile" || kind === "entity") parts.push("Sketch")
			if (kind === "profile" || kind === "entity") parts.push(this.navigationSelection[2] === 0 ? "Outline" : `Hole ${this.navigationSelection[2]}`)
			if (kind === "entity") {
				const loop = (this.loops.get(step.id) ?? stepLoops(this.draft, step))[Number(this.navigationSelection[2])]
				const primitive = loop && primitiveForLoop(this.draft, step, loop)
				parts.push(primitive ? readableName(primitive.type) : `Line ${Number(this.navigationSelection[3]) + 1}`)
			}
			const path = kind === "part" ? "Part" : parts.join(" › ")
			note(this.root, path)
			this.root.setAttribute("data-property-selection", navigationKey(...this.navigationSelection))
			if (this.navigationSelection[0] === "part") {
				note(this.root, "Choose a feature or sketch in the project tree, or click a border in the viewer.")
				this.navigation.change()
				return
			}
		}
		const props = section(this.root, `Edit ${step.id}`, true)
		field(props, "Feature name", step.id, (value) => {
			if (!value.trim() || steps.some((s) => s !== step && s.id === value)) {
				this.status.textContent = "Feature names must be unique and nonempty."
				return
			}
			const loops = this.loops.get(step.id)
			this.loops.delete(step.id)
			for (const dependent of steps) if (dependent.sourceId === step.id) dependent.sourceId = value
			step.id = value
			if (this.navigation) {
				this.navigationSelection[1] = value
				this.navigation.select(navigationKey(...this.navigationSelection))
			}
			if (loops) this.loops.set(value, loops)
			this.changed()
		})
		select(props, "Operation", step.operation, ["join", "cut", "intersect"], (v) => {
			step.operation = v as SolidStep["operation"]
			this.changed()
		})
		if (step.type === "loft") {
			const loft = requireValue(step.loft)
			note(props, "Loft between circular sketches on parallel XY planes. The first is at Z = 0; the second is on an offset plane.")
			for (const [label, key, factor] of [
				["Bottom sketch diameter (mm)", "bottomRadius", 2],
				["Top sketch diameter (mm)", "topRadius", 2],
				["Plane offset (mm)", "height", 1],
				["Loft segments", "segments", 1]
			] as const)
				numberField(props, label, loft[key] * factor, (n) => {
					loft[key] = n / factor
					this.changed()
				})
		} else if (step.type === "shell") {
			select(
				props,
				"Loft to shell",
				step.sourceId ?? "",
				steps
					.slice(0, this.selected)
					.filter((s) => s.type === "loft")
					.map((s) => s.id),
				(value) => {
					step.sourceId = value
					this.changed()
				}
			)
			note(props, "Remove the top face and offset the walls inward, preserving a bottom of the same thickness.")
			numberField(props, "Shell thickness (mm)", step.thickness ?? 1, (n) => {
				step.thickness = n
				this.changed()
			})
		} else if (step.type === "revolve") {
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
			select(
				props,
				"End condition",
				step.endCondition ?? "blind",
				[
					{ value: "blind", label: "Blind" },
					{ value: "through-all", label: "Through all (both directions)" }
				],
				(value) => {
					step.endCondition = value as "blind" | "through-all"
					this.render()
					this.changed()
				}
			)
			if (step.endCondition !== "through-all")
				numberField(props, "Extrusion depth (mm)", feature.depth, (n) => {
					feature.depth = n
					this.changed()
				})
			numberField(props, "Top scale", step.topScale ?? 1, (n) => {
				step.topScale = n
				this.changed()
			})
			const sketch = sketchFor(this.draft, step)
			this.renderSketchSupport(props, sketch, steps.slice(0, this.selected))
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
					if (!steps.some((s) => s !== step && s.featureId === step.featureId)) {
						this.draft.features = this.draft.features.filter((f) => f.id !== feature.id && !(f.type === "chamfer" && f.target.edge.extrudeId === feature.id))
					}
				}
				steps.splice(this.selected, 1)
				this.loops.delete(step.id)
				this.selected = Math.max(0, this.selected - 1)
				if (this.navigation) {
					this.navigationSelection = steps[this.selected] ? ["feature", requireValue(steps[this.selected]).id] : ["part"]
					this.navigation.select(navigationKey(...this.navigationSelection))
				}
				this.render()
				this.changed()
			})
		)
		if (step.type === "extrusion" || step.type === "revolve") {
			this.attempt(() => this.renderProfiles(step))
			if (!this.navigation || this.navigationSelection[0] === "feature") this.renderFinishes(step)
		}
		if (this.navigation) {
			props.hidden = this.navigationSelection[0] !== "feature"
			this.navigation.change()
		}
	}
	private openSketch(sketch: Sketch) {
		if (this.disposed) return
		this.workspace?.dispose()
		this.workspace = null
		// Flush property edits before handing ownership of geometry to the workspace.
		for (const step of this.draft.solidSteps ?? []) {
			if (step.type !== "extrusion" || extrusionFor(this.draft, step).target.sketchId !== sketch.id) continue
			const loops = this.loops.get(step.id)
			if (loops && sketch.relations === undefined) replaceStepLoops(this.draft, step, loops)
			this.loops.delete(step.id)
		}
		const current = this.draft.features.find((f): f is Sketch => f.type === "sketch" && f.id === sketch.id) ?? sketch
		let supportEdges: Point2D[][] = []
		let supportError: string | undefined
		try {
			supportEdges = sketchSupportEdges(this.draft, current.target)
		} catch (error) {
			supportError = error instanceof Error ? error.message : String(error)
		}
		const workspace = new SketchWorkspace(
			current,
			(next) => {
				if (this.disposed || this.workspace !== workspace) throw Error("This sketch editing session has closed.")
				const dependents = this.draft.features.filter((f) => f.type === "extrude" && f.target.sketchId === sketch.id)
				for (const feature of dependents)
					if (feature.type === "extrude" && !next.profiles.some((p) => p.id === feature.target.profileId))
						throw Error("A profile used by an extrusion was removed. Keep its closed region before finishing.")
				const index = this.draft.features.findIndex((f) => f.id === next.id)
				if (index < 0) this.draft.features.push(next)
				else this.draft.features[index] = next
				this.workspace = null
				this.navigationSelection = ["standalone-sketch", next.id]
				this.render()
				this.changed()
				this.navigation?.select(navigationKey(...this.navigationSelection))
			},
			() => {
				if (this.workspace === workspace) {
					this.workspace = null
					this.render()
				}
			},
			supportEdges,
			supportError
		)
		this.workspace = workspace
		if (this.sketchViewport) workspace.mount(this.sketchViewport, this.root)
		else this.root.append(workspace.root)
		workspace.focus()
	}
	private renderSketchSupport(parent: HTMLElement, sketch: Sketch, steps = this.draft.solidSteps ?? []) {
		const supports = new Map<string, SketchTarget>(["XY", "XZ", "YZ"].map((plane) => [plane, { type: "plane", plane: plane as SketchPlane }]))
		const choices = ["XY", "XZ", "YZ"].map((plane) => ({ value: plane, label: `${plane} plane` }))
		const dependsOnSketch = (featureId: string, visited = new Set<string>()): boolean => {
			if (visited.has(featureId)) return true
			visited.add(featureId)
			const feature = this.draft.features.find((feature) => feature.id === featureId)
			if (feature?.type !== "extrude") return true
			if (feature.target.sketchId === sketch.id) return true
			const source = this.draft.features.find((candidate) => candidate.id === feature.target.sketchId)
			return source?.type !== "sketch" || (source.target.type === "face" && dependsOnSketch(source.target.face.extrudeId, visited))
		}
		let selected = sketch.target.type === "plane" ? sketch.target.plane : "current-face"
		for (const step of steps) {
			if (step.type !== "extrusion") continue
			try {
				const feature = extrusionFor(this.draft, step)
				if (dependsOnSketch(feature.id)) continue
				const faces = getExtrudedFaceDescriptors(extrudeSolidFeature(this.draft, feature))
				faces.sort((a, b) => Number(a.label.startsWith("Side")) - Number(b.label.startsWith("Side")))
				for (const face of faces) {
					const value = `face-${supports.size}`
					supports.set(value, { type: "face", face: { type: "extrudeFace", extrudeId: feature.id, faceId: face.faceId } })
					choices.push({ value, label: `${step.id}: ${face.label}` })
					if (sketch.target.type === "face" && sketch.target.face.extrudeId === feature.id && sketch.target.face.faceId === face.faceId) selected = value
				}
			} catch {}
		}
		if (selected === "current-face") {
			supports.set(selected, structuredClone(sketch.target))
			choices.push({ value: selected, label: "Current face (unavailable)" })
		}
		select(parent, "Sketch support", selected, choices, (value) => {
			sketch.target = structuredClone(requireValue(supports.get(value)))
			this.changed()
		})
	}
	private renderSketchExtrusion(sketch: Sketch) {
		this.renderSketchSupport(this.root, sketch)
		const profiles = materializeSketch(sketch).profiles
		note(this.root, `${sketch.name ?? sketch.id}: ${profiles.length} closed region(s).`)
		if (!profiles.length) return
		let profileId = requireValue(profiles[0]).id
		let depth = 10
		select(
			this.root,
			"Extrusion region",
			profileId,
			profiles.map((p) => p.id),
			(value) => {
				profileId = value
			}
		)
		numberField(this.root, "New extrusion depth", depth, (value) => {
			depth = value
		})
		this.root.append(
			button("Extrude sketch", () =>
				this.attempt(() => {
					if (!Number.isFinite(depth) || depth <= 0) throw Error("Extrusion depth must be positive.")
					let id = "Extrude 1"
					for (let n = 2; this.draft.features.some((f) => f.id === id) || this.draft.solidSteps?.some((s) => s.id === id); n++) id = `Extrude ${n}`
					this.draft.features.push({ type: "extrude", id, depth, target: { type: "profileRef", sketchId: sketch.id, profileId } })
					this.draft.solidSteps ??= []
					this.draft.solidSteps.push({ id, type: "extrusion", featureId: id, operation: "join" })
					this.selected = this.draft.solidSteps.length - 1
					this.navigationSelection = ["feature", id]
					this.render()
					this.changed()
					this.navigation?.select(navigationKey(...this.navigationSelection))
				})
			)
		)
	}

	private renderProfiles(step: SolidStep) {
		if (step.type === "extrusion") {
			const sketch = sketchFor(this.draft, step)
			this.root.append(button("Edit sketch", () => this.openSketch(sketch)))
			if (sketch.relations !== undefined) {
				this.loops.delete(step.id)
				note(this.root, "Use Edit sketch to draw, edit dimensions, and solve this sketch.")
				return
			}
		}
		const loops = this.loops.get(step.id) ?? stepLoops(this.draft, step)
		this.loops.set(step.id, loops)
		const profiles = section(this.root, step.type === "revolve" ? "Revolve profile (radius / height)" : "Sketch profile and holes", true)
		if (this.navigation) {
			profiles.hidden = this.navigationSelection[0] === "feature"
			profiles.querySelector("summary")?.setAttribute("style", "display:none")
		}
		loops.forEach((points, index) => {
			const picked = this.source?.stepId === step.id && this.source.loopIndex === index
			const group = section(profiles, index === 0 ? "Outline" : `Hole ${index}`, index === 0 || picked)
			if (index > 0)
				group.append(
					button("Remove hole", () => {
						loops.splice(index, 1)
						this.render()
						this.changed()
					})
				)
			if (picked && this.source) {
				group.setAttribute("data-selected-source", "true")
				group.style.borderColor = "#d98200"
				group.style.background = "#fff4db"
				note(
					group,
					`Selected sketch: ${this.source.sketchId}. Entity: ${this.source.entityId ?? `profile segment ${this.source.edgeIndex}`}. Feature: ${step.id} (${step.operation}).`
				)
			}
			if (step.type === "extrusion") {
				const primitive = primitiveForLoop(this.draft, step, points)
				select(group, "Profile shape", primitive?.type ?? "polygon", ["circle", "capsule", "rectangle", "polygon"], (shape) =>
					this.attempt(() => {
						const outline =
							shape === "circle"
								? circlePrimitive(v2(0, 0), 10, 64)
								: shape === "capsule"
									? capsulePrimitive(v2(0, 0), v2(30, 0), 20, 32)
									: shape === "rectangle"
										? rectanglePrimitive(v2(0, 0), 20, 20)
										: rectangle(v2(0, 0), 20, 20)
						replaceStepOutline(this.draft, step, loops, index, outline)
						this.loops.delete(step.id)
						this.render()
						this.changed()
					})
				)
			}
			if (this.navigation) {
				group.hidden = (this.navigationSelection[0] === "profile" || this.navigationSelection[0] === "entity") && this.navigationSelection[2] !== index
				group.open = true
				group.querySelector("summary")?.setAttribute("style", "display:none")
				if (primitiveForLoop(this.draft, step, points)) {
					this.renderPrimitive(group, requireValue(primitiveForLoop(this.draft, step, points)), step)
					return
				}
				if (this.navigationSelection[0] === "entity" && !group.hidden) {
					const edge = Number(this.navigationSelection[3])
					const a = points[edge]
					const b = points[(edge + 1) % points.length]
					if (a && b) {
						note(group, `Line ${edge + 1} · ${index === 0 ? "Outline" : `Hole ${index}`}`)
						for (const [label, point] of [
							["Start", a],
							["End", b]
						] as const) {
							numberField(group, `${label} X`, point.x, (n) => {
								point.x = n
								this.changed()
							})
							numberField(group, `${label} Y`, point.y, (n) => {
								point.y = n
								this.changed()
							})
						}
						this.profileSvg(group, points, edge)
					}
					return
				}
			}
			const primitive = primitiveForLoop(this.draft, step, points)
			if (primitive) {
				this.renderPrimitive(group, primitive, step)
				return
			}
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
			this.profileSvg(group, points, picked ? this.source?.edgeIndex : undefined)
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
						if (step.type === "revolve") loops[index] = circle(v2(cx, cy), radius, segments)
						else replaceStepOutline(this.draft, step, loops, index, circlePrimitive(v2(cx, cy), radius, segments))
						if (step.type !== "revolve") this.loops.delete(step.id)
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
					if (step.type === "revolve") loops[index] = rectangle(v2(cx, cy), shapeWidth, shapeHeight)
					else replaceStepOutline(this.draft, step, loops, index, rectanglePrimitive(v2(cx, cy), shapeWidth, shapeHeight))
					if (step.type !== "revolve") this.loops.delete(step.id)
					this.render()
					this.changed()
				})
			)
			const vertices = section(group, `Vertices (${points.length}) — exact coordinates`)
			if (this.navigation) vertices.hidden = this.navigationSelection[0] === "entity"
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
		})
		if (step.type === "extrusion" && (!this.navigation || this.navigationSelection[0] === "sketch")) {
			const regions = section(profiles, "Additional filled regions", true)
			note(regions, "These shapes share the sketch plane, depth, and operation. Overlapping shapes fuse; separate shapes extrude together.")
			for (const [index, region] of (step.regions ?? []).entries()) {
				const group = section(regions, `Region ${index + 2}`, true)
				this.renderPrimitive(group, region, step, true)
				group.append(
					button("Remove region", () => {
						step.regions?.splice(index, 1)
						this.render()
						this.changed()
					})
				)
			}
			for (const shape of ["circle", "capsule", "rectangle"] as const)
				regions.append(
					button(`Add ${shape} region`, () => {
						step.regions ??= []
						let id = `region-${step.regions.length + 2}`
						while (step.regions.some((r) => r.id === id)) id += "-new"
						const shapeData =
							shape === "circle"
								? circlePrimitive(v2(30, 0), 5, 64)
								: shape === "capsule"
									? capsulePrimitive(v2(0, 0), v2(0, 30), 20, 32)
									: rectanglePrimitive(v2(30, 0), 20, 20)
						step.regions.push({ ...shapeData, id })
						this.render()
						this.changed()
					})
				)
		}
		if (step.type === "extrusion" && (!this.navigation || this.navigationSelection[0] === "sketch" || this.navigationSelection[0] === "profile"))
			profiles.append(
				button("Add circular hole", () => {
					loops.push(circle(v2(0, 0), 2, 32))
					replaceStepOutline(this.draft, step, loops, loops.length - 1, circlePrimitive(v2(0, 0), 2, 32))
					if (step.type !== "revolve") this.loops.delete(step.id)
					this.render()
					this.changed()
				})
			)
	}
	private renderPrimitive(parent: HTMLElement, primitive: SketchPrimitive, step: SolidStep, region = false): void {
		note(parent, readableName(primitive.type))
		const update = (change: () => void) =>
			this.attempt(() => {
				const previous = structuredClone(primitive)
				// Commit pending polygon edits before changing the primitive's parameters.
				const loops = this.loops.get(step.id)
				if (loops) replaceStepLoops(this.draft, step, loops)
				const entity = region ? primitive : sketchFor(this.draft, step).entities.find((e) => e.id === primitive.id)
				if (!entity) throw Error("Missing primitive")
				change()
				if (!normalizePrimitive(primitive, primitive.id)) {
					Object.assign(primitive, previous)
					throw Error("Use finite coordinates and positive primitive dimensions.")
				}
				Object.assign(entity, structuredClone(primitive))
				this.loops.delete(step.id)
				this.draft.cad = undefined
				this.draft.tree = undefined
				this.render()
				this.changed()
			})
		const coordinate = (label: string, point: Point2D, key: "x" | "y") =>
			numberField(parent, label, point[key], (n) =>
				update(() => {
					point[key] = n
				})
			)
		if (primitive.type === "capsule") {
			coordinate("Start center X", primitive.from, "x")
			coordinate("Start center Y", primitive.from, "y")
			coordinate("End center X", primitive.to, "x")
			coordinate("End center Y", primitive.to, "y")
			numberField(parent, "Capsule width (mm)", primitive.width, (n) =>
				update(() => {
					primitive.width = n
				})
			)
		} else {
			coordinate("Center X", primitive.center, "x")
			coordinate("Center Y", primitive.center, "y")
			if (primitive.type === "circle")
				numberField(parent, "Diameter (mm)", primitive.radius * 2, (n) =>
					update(() => {
						primitive.radius = n / 2
					})
				)
			else if (primitive.type === "rectangle") {
				numberField(parent, "Rectangle width (mm)", primitive.width, (n) =>
					update(() => {
						primitive.width = n
					})
				)
				numberField(parent, "Rectangle height (mm)", primitive.height, (n) =>
					update(() => {
						primitive.height = n
					})
				)
				numberField(parent, "Rectangle rotation (deg)", primitive.rotation, (n) =>
					update(() => {
						primitive.rotation = n
					})
				)
			}
		}
		if (primitive.type === "circle" || primitive.type === "capsule") {
			const label = primitive.type === "circle" ? "Circle segments" : "Capsule arc segments"
			numberField(parent, label, primitive.type === "circle" ? primitive.segments : primitive.arcSegments, (n) =>
				update(() => {
					if (primitive.type === "circle") primitive.segments = n
					else primitive.arcSegments = n
				})
			)
		}
		this.profileSvg(parent, primitivePoints(primitive), undefined, true)
	}

	private profileSvg(parent: HTMLElement, points: Point2D[], selectedEdge?: number, readOnly = false) {
		const ns = "http://www.w3.org/2000/svg"
		const svg = document.createElementNS(ns, "svg")
		svg.setAttribute("viewBox", "0 0 300 190")
		svg.style.cssText = "width:100%;height:190px;background:#eaf0f7;touch-action:none;border:1px solid #cbd5e1"
		svg.setAttribute("aria-label", readOnly ? "Primitive preview; edit its dimensions above." : "Profile preview; drag a vertex to edit, or use exact coordinate fields below.")
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
		if (readOnly && parent.hasAttribute("data-selected-source")) {
			polygon.setAttribute("aria-label", "Selected sketch entity")
			polygon.setAttribute("stroke", "#e68a00")
		}
		svg.append(polygon)
		if (selectedEdge !== undefined) {
			const a = points[selectedEdge]
			const b = points[(selectedEdge + 1) % points.length]
			if (a && b) {
				const line = document.createElementNS(ns, "line")
				line.setAttribute("x1", String(150 + (a.x - cx) * scale))
				line.setAttribute("y1", String(95 - (a.y - cy) * scale))
				line.setAttribute("x2", String(150 + (b.x - cx) * scale))
				line.setAttribute("y2", String(95 - (b.y - cy) * scale))
				line.setAttribute("stroke", "#e68a00")
				line.setAttribute("stroke-width", "4")
				line.setAttribute("aria-label", "Selected sketch entity")
				svg.append(line)
			}
		}
		const draw = () => polygon.setAttribute("points", points.map((p) => `${150 + (p.x - cx) * scale},${95 - (p.y - cy) * scale}`).join(" "))
		draw()
		if (!readOnly)
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
