import { describe, expect, it } from "bun:test"
import { deserialize, type SerializedMap } from "../src/serilization"
import { Component, Net, Pin, Schematic } from "../src/puppycad"

describe("deserialize", () => {
	it("reconstructs relationships between schematics, components, nets, and pins", () => {
		const serialized: SerializedMap = {
			"schematic-1": {
				type: "schematic",
				id: "schematic-1",
				name: "Main",
				nets: ["net-1", 42, null],
				components: ["component-1", { id: "not-a-string" } as unknown as string]
			},
			"component-1": {
				type: "component",
				id: "component-1",
				name: "Resistor",
				pins: ["pin-1", 99 as unknown as string]
			},
			"pin-1": {
				type: "pin",
				id: "pin-1",
				name: "1",
				componentId: "component-1"
			},
			"net-1": {
				type: "net",
				id: "net-1",
				name: "GND",
				pins: ["pin-1", undefined as unknown as string]
			},
			ignored: {
				type: "custom",
				id: "ignored"
			}
		}

		const entities = deserialize(serialized)

		const schematic = entities.find((entity): entity is Schematic => entity instanceof Schematic)
		const component = entities.find((entity): entity is Component => entity instanceof Component)
		const net = entities.find((entity): entity is Net => entity instanceof Net)
		const pin = entities.find((entity): entity is Pin => entity instanceof Pin)

		expect(schematic).toBeDefined()
		expect(component).toBeDefined()
		expect(net).toBeDefined()
		expect(pin).toBeDefined()

		if (!schematic || !component || !net || !pin) {
			throw new Error("Expected entities to be defined")
		}

		expect(schematic.components).toHaveLength(1)
		expect(schematic.components[0]).toBe(component)

		expect(schematic.nets).toHaveLength(1)
		expect(schematic.nets[0]).toBe(net)

		expect(component.pins).toHaveLength(1)
		expect(component.pins[0]).toBe(pin)
		expect(pin.component).toBe(component)

		expect(net.pins).toHaveLength(1)
		expect(net.pins[0]).toBe(pin)
	})

	it("falls back to default identifiers and ignores invalid references", () => {
		const serialized: SerializedMap = {
			"component-without-id": {
				type: "component",
				name: 123 as unknown as string,
				pins: "pin-2" as unknown as string[]
			},
			"pin-2": {
				type: "pin",
				name: { text: "invalid" } as unknown as string
			}
		}

		const entities = deserialize(serialized)

		const component = entities.find((entity): entity is Component => entity instanceof Component)
		const pin = entities.find((entity): entity is Pin => entity instanceof Pin)

		expect(component).toBeDefined()
		expect(pin).toBeDefined()

		if (!component || !pin) {
			throw new Error("Expected component and pin to be defined")
		}

		expect(component.id).toBe("component-without-id")
		expect(component.name).toBe("Component")
		expect(component.pins).toHaveLength(0)

		expect(pin.id).toBe("pin-2")
		expect(pin.name).toBe("Pin")
		expect(pin.component).toBeUndefined()
	})
})
