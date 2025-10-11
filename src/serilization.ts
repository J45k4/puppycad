import { Component, Net, Pin, Schematic, type Entity } from "./puppycad"

export type SerializedEntity = {
	type: string
	[key: string]: unknown
}

export type SerializedMap = Record<string, SerializedEntity>

export const serialize = (entities: Entity[] | Entity) => {
	const map: SerializedMap = {}
	const visited = new Set<Entity>()

	const serializeEntity = (entity: Entity) => {
		entity.visit((entity) => {
			map[entity.id] = entity.serialize()
		}, visited)
	}

	if (Array.isArray(entities)) {
		for (const entity of entities) {
			serializeEntity(entity)
		}
	} else {
		serializeEntity(entities)
	}

	return map
}

export const deserialize = (data: SerializedMap): Entity[] => {
	const entityMap = new Map<string, Entity>()
	const pendingComponents: Array<{ id: string; pinIds: string[] }> = []
	const pendingNets: Array<{ id: string; pinIds: string[] }> = []
	const pendingSchematics: Array<{ id: string; netIds: string[]; componentIds: string[] }> = []
	const pinComponentLinks: Array<{ pinId: string; componentId: string }> = []
	const toStringArray = (value: unknown): string[] => {
		if (!Array.isArray(value)) {
			return []
		}
		const items: string[] = []
		for (const entry of value) {
			if (typeof entry === "string") {
				items.push(entry)
			}
		}
		return items
	}

	for (const [id, value] of Object.entries(data)) {
		const entityId = typeof value.id === "string" ? value.id : id
		switch (value.type) {
			case "schematic":
				entityMap.set(
					id,
					new Schematic(
						{
							name: typeof value.name === "string" ? value.name : undefined,
							nets: [],
							components: []
						},
						entityId
					)
				)
				pendingSchematics.push({
					id,
					netIds: toStringArray(value.nets),
					componentIds: toStringArray(value.components)
				})
				break
			case "net":
				entityMap.set(id, new Net(typeof value.name === "string" ? value.name : "Net", entityId))
				pendingNets.push({
					id,
					pinIds: toStringArray(value.pins)
				})
				break
			case "component":
				entityMap.set(id, new Component(typeof value.name === "string" ? value.name : "Component", entityId))
				pendingComponents.push({
					id,
					pinIds: toStringArray(value.pins)
				})
				break
			case "pin":
				entityMap.set(id, new Pin(typeof value.name === "string" ? value.name : "Pin", entityId))
				if (typeof value.componentId === "string") {
					pinComponentLinks.push({ pinId: id, componentId: value.componentId })
				}
				break
			default:
				continue
		}
	}

	for (const { id, pinIds } of pendingComponents) {
		const component = entityMap.get(id)
		if (!(component instanceof Component)) {
			continue
		}
		for (const pinId of pinIds) {
			const pin = entityMap.get(pinId)
			if (pin instanceof Pin) {
				component.addPin(pin)
			}
		}
	}

	for (const { pinId, componentId } of pinComponentLinks) {
		const pin = entityMap.get(pinId)
		const component = entityMap.get(componentId)
		if (pin instanceof Pin && component instanceof Component) {
			component.addPin(pin)
		}
	}

	for (const { id, pinIds } of pendingNets) {
		const net = entityMap.get(id)
		if (!(net instanceof Net)) {
			continue
		}
		for (const pinId of pinIds) {
			const pin = entityMap.get(pinId)
			if (pin instanceof Pin) {
				net.connect(pin)
			}
		}
	}

	for (const { id, netIds, componentIds } of pendingSchematics) {
		const schematic = entityMap.get(id)
		if (!(schematic instanceof Schematic)) {
			continue
		}
		for (const netId of netIds) {
			const net = entityMap.get(netId)
			if (net instanceof Net) {
				schematic.addNet(net)
			}
		}
		for (const componentId of componentIds) {
			const component = entityMap.get(componentId)
			if (component instanceof Component) {
				schematic.addComponent(component)
			}
		}
	}

	return Array.from(entityMap.values())
}
