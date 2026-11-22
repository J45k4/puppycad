import { navigationKey, readableName, type ModelNavigationNode } from "./model-navigation"
import type { Assembly, AssemblyConnector, ProjectNode } from "../contract"
import { solveFixedAssembly, transformMatrix } from "../assembly-solver"
import { button, field, note, panelStyle, section, select, vector, validateFields } from "./model-fields"
export function partChoices(nodes: ProjectNode[]): { value: string; label: string }[] {
	return nodes.flatMap((n) => ("kind" in n ? partChoices(n.items) : n.type === "part" ? [{ value: n.id, label: n.name }] : []))
}
export function validateAssemblyEdit(input: Assembly, parts: ProjectNode[]): Assembly {
	const ids = (values: { id: string }[], kind: string) => {
		const seen = new Set<string>()
		for (const value of values) {
			if (!value.id.trim() || seen.has(value.id)) throw Error(`${kind} names must be unique and nonempty.`)
			seen.add(value.id)
		}
	}
	ids(input.instances, "Instance")
	ids(input.connectors ?? [], "Connector")
	ids(input.mates ?? [], "Connection")
	const available = new Set(partChoices(parts).map((p) => p.value))
	const instances = new Set(input.instances.map((i) => i.id))
	for (const instance of input.instances) {
		if (!available.has(instance.partId)) throw Error(`Missing part: ${instance.partId}`)
		transformMatrix(instance.transform)
	}
	for (const connector of input.connectors ?? []) {
		if (connector.instanceId !== null && !instances.has(connector.instanceId)) throw Error(`Missing instance for connector ${connector.id}`)
		transformMatrix({ translation: connector.position, rotation: connector.rotation })
	}
	for (const mate of input.mates ?? [])
		for (const ref of [mate.a, mate.b]) {
			const connector = input.connectors?.find((c) => c.id === ref.connectorId)
			if (!connector || connector.instanceId !== ref.instanceId) throw Error(`Missing or mismatched connector in connection ${mate.id}`)
		}
	return solveFixedAssembly(input)
}
export class AssemblyProperties {
	readonly root = document.createElement("aside")
	private draft: Assembly
	private status = document.createElement("p")
	private dirty = false
	private selectedInstance: string | null = null
	private navigation?: { select: (key: string) => void; change: () => void }
	private navigationSelection = ["assembly"]
	public useProjectNavigation(select: (key: string) => void, change: () => void): void {
		this.navigation = { select, change }
		this.root.style.cssText = "height:100%;overflow:auto;padding:12px;box-sizing:border-box;min-width:0;background:#f8fafc"
		this.render()
	}
	public getNavigation(): ModelNavigationNode[] {
		return [
			{
				key: navigationKey("instances"),
				label: "Instances",
				children: this.draft.instances.map((i) => ({
					key: navigationKey("instance", i.id),
					label: readableName(i.id),
					children: [
						{
							key: navigationKey("definition", i.partId, i.id),
							label: `Part: ${partChoices(this.getParts()).find((p) => p.value === i.partId)?.label ?? i.partId}`
						}
					]
				}))
			},
			{
				key: navigationKey("connectors"),
				label: "Connectors",
				children: (this.draft.connectors ?? []).map((c) => ({ key: navigationKey("connector", c.id), label: c.name ?? readableName(c.id) }))
			},
			{
				key: navigationKey("connections"),
				label: "Connections",
				children: (this.draft.mates ?? []).map((m) => ({ key: navigationKey("connection", m.id), label: m.name ?? readableName(m.id) }))
			}
		]
	}
	public selectNavigation(key: string): void {
		this.navigationSelection = JSON.parse(key)
		if (this.navigationSelection[0] === "definition") {
			this.openPart?.(this.navigationSelection[1] ?? "")
			return
		}
		this.render()
		this.onSelectInstance?.(this.navigationSelection[0] === "instance" ? (this.navigationSelection[1] ?? null) : null)
	}
	private filterProperties(): void {
		if (!this.navigation) return
		this.root.setAttribute("data-property-selection", navigationKey(...this.navigationSelection))
		for (const group of Array.from(this.root.querySelectorAll<HTMLElement>("[data-navigation-group]"))) {
			const kind = this.navigationSelection[0]
			group.hidden = kind !== "assembly" && kind !== group.dataset.navigationGroup && kind !== `${group.dataset.navigationGroup}s`
			for (const action of Array.from(group.children)) if (action.tagName === "BUTTON") (action as HTMLElement).hidden = kind === group.dataset.navigationGroup
		}
		const breadcrumb = this.root.querySelector("[data-property-path]")
		if (breadcrumb) breadcrumb.textContent = this.navigationSelection.map(readableName).join(" › ")
		for (const row of Array.from(this.root.querySelectorAll<HTMLElement>("[data-navigation-key]"))) row.hidden = row.dataset.navigationKey !== navigationKey(...this.navigationSelection)
	}

	private instanceRows = new Map<string, HTMLDetailsElement>()
	constructor(
		private accepted: Assembly,
		private getParts: () => ProjectNode[],
		private apply: (next: Assembly) => void,
		private openPart?: (id: string) => void,
		private onSelectInstance?: (id: string | null) => void
	) {
		this.draft = structuredClone(accepted)
		this.root.style.cssText = panelStyle
		this.root.setAttribute("aria-label", "Assembly properties")
		this.render()
	}
	public refreshChoices(): void {
		this.render()
	}
	public selectInstance(id: string | null): void {
		this.selectedInstance = id
		if (!id && this.navigation && this.navigationSelection[0] === "instance") {
			this.navigationSelection = ["assembly"]
			this.filterProperties()
			this.navigation.select(navigationKey("assembly"))
		}
		if (id && this.navigation) {
			this.navigationSelection = ["instance", id]
			this.filterProperties()
			this.navigation.select(navigationKey("instance", id))
		}
		for (const [instanceId, row] of this.instanceRows) {
			const selected = instanceId === id
			row.style.borderColor = selected ? "#246bd1" : "#cbd5e1"
			row.style.background = selected ? "#e8f0ff" : "transparent"
			if (!this.navigation) row.querySelector("button")?.setAttribute("aria-pressed", String(selected))
			if (selected) {
				row.open = true
				if (row.parentElement?.tagName === "DETAILS") (row.parentElement as HTMLDetailsElement).open = true
				row.scrollIntoView?.({ block: "nearest" })
			}
		}
	}
	private changed = () => {
		this.dirty = true
		this.status.textContent = "Unsaved assembly changes."
		this.navigation?.change()
	}
	private render() {
		this.root.replaceChildren()
		this.instanceRows.clear()
		const title = document.createElement("h2")
		title.textContent = this.navigation ? "Properties" : "Assembly"
		title.style.margin = "0"
		this.root.append(title)
		if (this.navigation) {
			const path = document.createElement("p")
			path.setAttribute("data-property-path", "")
			this.root.append(path)
		}
		note(
			this.root,
			"Positions are millimetres; rotations are degrees. Fixed connections control connected parts: edit connector frames to change their relative positions. Edit all affected connections together, then Apply."
		)
		this.root.append(
			button("Apply assembly changes", () => {
				try {
					validateFields(this.root)
					const next = validateAssemblyEdit(this.draft, this.getParts())
					this.apply(next)
					this.dirty = false
					this.accepted = structuredClone(next)
					this.draft = structuredClone(next)
					this.render()
					this.status.textContent = "Assembly changes applied."
				} catch (error) {
					this.status.textContent = error instanceof Error ? error.message : String(error)
					this.status.setAttribute("role", "alert")
				}
			}),
			button("Discard assembly changes", () => {
				this.dirty = false
				this.draft = structuredClone(this.accepted)
				this.render()
			})
		)
		this.status = document.createElement("p")
		this.status.setAttribute("role", "status")
		if (this.dirty) this.status.textContent = "Unsaved changes — Apply or Discard."
		this.status.style.cssText = "font-size:13px;color:#a04115"
		this.root.append(this.status)
		const parts = partChoices(this.getParts())
		const instances = section(this.root, `Instances (${this.draft.instances.length})`, true)
		instances.dataset.navigationGroup = "instance"
		for (const instance of this.draft.instances) {
			const row = section(instances, instance.id)
			row.dataset.navigationKey = navigationKey("instance", instance.id)
			this.instanceRows.set(instance.id, row)
			const pick = button(`Select ${instance.id}`, () => {
				this.selectInstance(instance.id)
				this.onSelectInstance?.(instance.id)
			})
			pick.setAttribute("aria-pressed", String(this.selectedInstance === instance.id))
			if (!this.navigation) row.append(pick)
			field(row, "Instance name", instance.id, (v) => {
				if (!v.trim() || this.draft.instances.some((i) => i !== instance && i.id === v)) {
					this.status.textContent = "Instance names must be unique and nonempty."
					return
				}
				const old = instance.id
				instance.id = v
				if (this.navigation) {
					this.navigationSelection = ["instance", v]
					row.dataset.navigationKey = navigationKey("instance", v)
					this.navigation.select(navigationKey("instance", v))
				}
				for (const c of this.draft.connectors ?? []) if (c.instanceId === old) c.instanceId = v
				for (const m of this.draft.mates ?? []) for (const ref of [m.a, m.b]) if (ref.instanceId === old) ref.instanceId = v
				this.changed()
			})
			select(row, "Part definition", instance.partId, parts, (v) => {
				instance.partId = v
				this.changed()
			})
			if (this.openPart) row.append(button("Edit part geometry", () => this.openPart?.(instance.partId)))
			instance.transform ??= {}
			const transform = instance.transform
			transform.translation ??= { x: 0, y: 0, z: 0 }
			transform.rotation ??= { x: 0, y: 0, z: 0 }
			transform.scale ??= { x: 1, y: 1, z: 1 }
			vector(row, "Position (mm)", transform.translation, this.changed)
			vector(row, "Rotation (degrees)", transform.rotation, this.changed)
			vector(row, "Scale", transform.scale, this.changed)
			row.append(
				button("Remove instance and connections", () => {
					this.draft.instances = this.draft.instances.filter((i) => i !== instance)
					const connectorIds = new Set(this.draft.connectors?.filter((c) => c.instanceId === instance.id).map((c) => c.id))
					this.draft.connectors = this.draft.connectors?.filter((c) => c.instanceId !== instance.id)
					this.removeMates(
						(m) => m.a.instanceId === instance.id || m.b.instanceId === instance.id || connectorIds.has(m.a.connectorId) || connectorIds.has(m.b.connectorId)
					)
					this.render()
					this.changed()
				})
			)
		}
		instances.append(
			button("Add instance", () => {
				const part = parts[0]
				if (!part) {
					this.status.textContent = "Create a part first."
					return
				}
				this.draft.instances.push({ id: this.unique("instance", this.draft.instances), partId: part.value })
				this.render()
				this.changed()
			})
		)
		const connectors = section(this.root, `Connectors (${this.draft.connectors?.length ?? 0})`)
		connectors.dataset.navigationGroup = "connector"
		for (const connector of this.draft.connectors ?? []) {
			const row = section(connectors, connector.name ?? connector.id)
			row.dataset.navigationKey = navigationKey("connector", connector.id)
			field(row, "Connector name", connector.id, (v) => {
				if (!v.trim() || this.draft.connectors?.some((c) => c !== connector && c.id === v)) {
					this.status.textContent = "Connector names must be unique and nonempty."
					return
				}
				const old = connector.id
				connector.id = v
				if (this.navigation) {
					this.navigationSelection = ["connector", v]
					row.dataset.navigationKey = navigationKey("connector", v)
					this.navigation.select(navigationKey("connector", v))
				}
				for (const m of this.draft.mates ?? []) for (const ref of [m.a, m.b]) if (ref.connectorId === old) ref.connectorId = v
				this.changed()
			})
			select(row, "Attached to", connector.instanceId ?? "", [{ value: "", label: "World (ground)" }, ...this.draft.instances.map((i) => ({ value: i.id, label: i.id }))], (v) => {
				connector.instanceId = v || null
				for (const mate of this.draft.mates ?? []) for (const ref of [mate.a, mate.b]) if (ref.connectorId === connector.id) ref.instanceId = connector.instanceId
				this.changed()
			})
			connector.rotation ??= { x: 0, y: 0, z: 0 }
			vector(row, "Connector position (mm)", connector.position, this.changed)
			vector(row, "Connector rotation (degrees)", connector.rotation, this.changed)
			row.append(
				button("Remove connector and connections", () => {
					this.draft.connectors = this.draft.connectors?.filter((c) => c !== connector)
					this.removeMates((m) => m.a.connectorId === connector.id || m.b.connectorId === connector.id)
					this.render()
					this.changed()
				})
			)
		}
		connectors.append(
			button("Add connector", () => {
				this.draft.connectors ??= []
				this.draft.connectors.push({ id: this.unique("connector", this.draft.connectors), instanceId: this.draft.instances[0]?.id ?? null, position: { x: 0, y: 0, z: 0 } })
				this.render()
				this.changed()
			})
		)
		const mates = section(this.root, `Fixed connections (${this.draft.mates?.length ?? 0})`)
		mates.dataset.navigationGroup = "connection"
		const refs = (this.draft.connectors ?? []).map((c) => ({ value: c.id, label: `${c.instanceId ?? "World"} / ${c.id}` }))
		for (const mate of this.draft.mates ?? []) {
			const row = section(mates, mate.name ?? mate.id)
			row.dataset.navigationKey = navigationKey("connection", mate.id)
			field(row, "Connection name", mate.id, (v) => {
				if (!v.trim() || this.draft.mates?.some((m) => m !== mate && m.id === v)) {
					this.status.textContent = "Connection names must be unique and nonempty."
					return
				}
				const old = mate.id
				mate.id = v
				if (this.navigation) {
					this.navigationSelection = ["connection", v]
					row.dataset.navigationKey = navigationKey("connection", v)
					this.navigation.select(navigationKey("connection", v))
				}
				for (const actuator of this.draft.actuators ?? []) if (actuator.mateId === old) actuator.mateId = v
				this.changed()
			})
			for (const side of ["a", "b"] as const)
				select(row, `Connector ${side.toUpperCase()}`, mate[side].connectorId, refs, (v) => {
					const c = this.draft.connectors?.find((c) => c.id === v)
					if (c) {
						mate[side] = { instanceId: c.instanceId, connectorId: c.id }
						this.changed()
					}
				})
			note(row, `Type: ${mate.type}. Fixed connections align both position and orientation.`)
			row.append(
				button("Remove connection", () => {
					this.removeMates((m) => m === mate)
					this.render()
					this.changed()
				})
			)
		}
		mates.append(
			button("Add fixed connection", () => {
				const a = this.draft.connectors?.[0]
				const b = this.draft.connectors?.find((c) => c.instanceId !== a?.instanceId)
				if (!a || !b) {
					this.status.textContent = "Add connectors on two different instances (or world) first."
					return
				}
				this.draft.mates ??= []
				const ref = (c: AssemblyConnector) => ({ instanceId: c.instanceId, connectorId: c.id })
				this.draft.mates.push({ id: this.unique("connection", this.draft.mates), type: "fasten", a: ref(a), b: ref(b) })
				this.render()
				this.changed()
			})
		)
		if (this.navigation) {
			const actions = Array.from(this.root.children)
				.filter((node) => node.tagName === "BUTTON")
				.slice(0, 2)
			if (actions.length) {
				const bar = document.createElement("div")
				bar.style.cssText = "position:sticky;top:0;z-index:2;background:#f8fafc;padding:8px 0;display:flex;gap:6px;flex-wrap:wrap"
				this.root.insertBefore(bar, actions[0] ?? null)
				bar.append(...actions)
			}
			for (const group of [instances, connectors, mates]) {
				group.open = true
				group.querySelector("summary")?.setAttribute("style", "display:none")
				for (const row of Array.from(group.querySelectorAll<HTMLDetailsElement>("[data-navigation-key]"))) {
					row.open = true
					row.querySelector("summary")?.setAttribute("style", "display:none")
				}
			}
			this.filterProperties()
			this.navigation.change()
		} else this.selectInstance(this.selectedInstance)
	}
	private unique(prefix: string, values: { id: string }[]): string {
		let id = `${prefix}-${values.length + 1}`
		while (values.some((v) => v.id === id)) id += "-new"
		return id
	}
	private removeMates(predicate: (mate: NonNullable<Assembly["mates"]>[number]) => boolean) {
		const removed = new Set(this.draft.mates?.filter(predicate).map((m) => m.id))
		this.draft.mates = this.draft.mates?.filter((m) => !removed.has(m.id))
		this.draft.actuators = this.draft.actuators?.filter((a) => !removed.has(a.mateId))
	}
}
