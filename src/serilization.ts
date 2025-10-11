import { Component, Entity, Net, Pin, Schematic } from "./puppycad"

export type SerializedMap = {
	[key: string]: any
}

export const serialize = (entities: Entity[] | Entity) => {
	const map: Record<string, any> = {}
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

	for (const [id, value] of Object.entries(data)) {
		let entity: Entity
		switch (value.type) {
			case "schematic":
				entity = Schematic.parse(value)
				break
			case "net":
				entity = Net.parse(value)
				break
			case "component":
				entity = Component.parse(value)
				break
			case "pin":
				const component = entityMap.get(value.componentId) as Component
				entity = Pin.parse(value, component)
				break
			default:
				continue
		}
		entityMap.set(id, entity)
	}

	return Array.from(entityMap.values())
}
