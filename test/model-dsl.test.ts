import { describe, expect, it } from "bun:test"
import handModel from "../examples/three-finger-hand.pcad.ts"
import { extrudeSolidFeature } from "../src/cad/extrude"
import { capsule, circle, compileModel, component, defineModel, serializeModelGraph, validateModel, v2 } from "../src/model-dsl"
import { normalizeProjectFile } from "../src/project-file"

describe("TypeScript model DSL", () => {
	it("creates reusable components with stable hierarchical ids and planar transforms", () => {
		const link = component<{ length: number }, { endFrameId: string }>("Link", (scope, props) => {
			const body = scope.body("body", {
				outline: capsule(v2(0, 0), v2(props.length, 0), 2),
				depth: 3
			})
			const end = scope.frame("end", { body, at: v2(props.length, 0) })
			return { endFrameId: end.id }
		})
		const model = defineModel({ id: "test-arm", name: "Test arm" }, (arm) => {
			arm.instance("upper", link, { length: 10 }, { translate: v2(2, 3), rotateDeg: 90 })
			arm.instance("lower", link, { length: 8 }, { translate: v2(2, 13) })
		})

		expect(model.components.map((entry) => entry.id)).toEqual(["test-arm", "test-arm/upper", "test-arm/lower"])
		expect(model.bodies.map((body) => body.id)).toEqual(["test-arm/upper/body", "test-arm/lower/body"])
		const upperEnd = model.frames.find((frame) => frame.id === "test-arm/upper/end")?.position
		expect(upperEnd?.x).toBeCloseTo(2)
		expect(upperEnd?.y).toBeCloseTo(13)
		expect(upperEnd?.z).toBe(0)
	})

	it("compiles the three-finger model into evaluable PuppyCAD geometry", () => {
		expect(handModel.components).toHaveLength(5)
		expect(handModel.bodies).toHaveLength(48)
		expect(handModel.frames).toHaveLength(39)
		expect(handModel.mates.filter((mate) => mate.type === "revolute")).toHaveLength(12)
		expect(handModel.mates.filter((mate) => mate.type === "fixed")).toHaveLength(6)
		expect(handModel.servos).toHaveLength(3)
		expect(handModel.servos.map((servo) => servo.id)).toEqual(["three-finger-hand/index/drive", "three-finger-hand/middle/drive", "three-finger-hand/thumb/drive"])

		const compiled = compileModel(handModel)
		expect(compiled.project.version).toBe(4)
		expect(compiled.project.items).toHaveLength(49)
		expect(compiled.project.selectedPath).toEqual([48])
		const assembly = compiled.project.items.find((item) => "type" in item && item.type === "assembly")
		if (!assembly || !("type" in assembly) || assembly.type !== "assembly" || !assembly.data) {
			throw new Error("Expected compiled assembly")
		}
		expect(assembly.id).toBe("three-finger-hand/assembly")
		expect(assembly.data.instances).toHaveLength(48)
		expect(assembly.data.connectors).toHaveLength(39)
		expect(assembly.data.mates).toHaveLength(18)
		expect(assembly.data.actuators).toHaveLength(3)
		expect(assembly.data.mates?.find((mate) => mate.id === "three-finger-hand/index/mcp")).toMatchObject({
			type: "revolute",
			a: {
				instanceId: "three-finger-hand/palm/servo-index",
				connectorId: "three-finger-hand/palm/servo-index-axis"
			},
			b: {
				instanceId: "three-finger-hand/index/proximal",
				connectorId: "three-finger-hand/index/proximal-mcp"
			}
		})
		expect(assembly.data.actuators?.[0]).toMatchObject({
			type: "servo",
			mateId: "three-finger-hand/index/mcp",
			commandRange: { minDeg: 0, maxDeg: 70 }
		})
		const instanceIds = new Set(assembly.data.instances.map((instance) => instance.id))
		const connectorById = new Map(assembly.data.connectors?.map((connector) => [connector.id, connector] as const))
		const mateIds = new Set(assembly.data.mates?.map((mate) => mate.id))
		for (const mate of assembly.data.mates ?? []) {
			for (const reference of [mate.a, mate.b]) {
				if (reference.instanceId !== null) {
					expect(instanceIds.has(reference.instanceId)).toBeTrue()
				}
				expect(connectorById.get(reference.connectorId)?.instanceId).toBe(reference.instanceId)
			}
		}
		for (const actuator of assembly.data.actuators ?? []) {
			expect(mateIds.has(actuator.mateId)).toBeTrue()
		}
		const normalized = normalizeProjectFile(JSON.parse(JSON.stringify(compiled.project)))
		const normalizedAssembly = normalized?.items.find((item) => "type" in item && item.type === "assembly")
		expect(normalizedAssembly).toMatchObject({
			id: "three-finger-hand/assembly",
			type: "assembly",
			data: {
				instances: expect.any(Array),
				connectors: expect.any(Array),
				mates: expect.any(Array),
				actuators: expect.any(Array)
			}
		})
		const solids = compiled.project.items.flatMap((item) => {
			if (!("type" in item) || item.type !== "part" || !item.data) {
				return []
			}
			const extrude = item.data.features.find((feature) => feature.type === "extrude")
			return extrude?.type === "extrude" ? [extrudeSolidFeature(item.data, extrude).solid] : []
		})
		expect(solids).toHaveLength(48)

		const positions = solids.flatMap((solid) => solid.vertices.map((vertex) => vertex.position))
		const xs = positions.map((point) => point.x)
		const ys = positions.map((point) => point.y)
		expect(Math.min(...xs)).toBeCloseTo(-49)
		expect(Math.max(...xs)).toBeCloseTo(87.2894, 3)
		expect(Math.min(...ys)).toBeCloseTo(-82.5)
		expect(Math.max(...ys)).toBeCloseTo(126.6239, 3)
	})

	it("keeps simulation metadata in a deterministic serializable graph", () => {
		const parsed = JSON.parse(serializeModelGraph(handModel)) as typeof handModel
		expect(parsed.kind).toBe("puppycad.model/v1")
		expect(parsed.mates.find((mate) => mate.id === "three-finger-hand/index/mcp")).toMatchObject({
			type: "revolute",
			parentFrameId: "three-finger-hand/palm/servo-index-axis",
			childFrameId: "three-finger-hand/index/proximal-mcp",
			limits: { minDeg: -10, maxDeg: 75 }
		})
		expect(parsed.servos[0]).toMatchObject({
			jointId: "three-finger-hand/index/mcp",
			commandRange: { minDeg: 0, maxDeg: 70 }
		})
	})

	it("rejects duplicate ids and invalid actuator ranges while building", () => {
		expect(() =>
			defineModel({ id: "bad", name: "Bad model" }, (model) => {
				model.body("same", { outline: circle(v2(0, 0), 2), depth: 1 })
				model.body("same", { outline: circle(v2(0, 0), 3), depth: 1 })
			})
		).toThrow('Duplicate body id "bad/same"')
	})

	it("rejects invalid limits and actuator values in externally supplied graphs", () => {
		const invalidLimits = structuredClone(handModel)
		const revolute = invalidLimits.mates.find((mate) => mate.type === "revolute")
		if (!revolute || revolute.type !== "revolute") {
			throw new Error("Expected a revolute mate")
		}
		revolute.limits = { minDeg: 20, maxDeg: 10 }
		expect(() => validateModel(invalidLimits)).toThrow("minimum limit cannot exceed its maximum limit")

		const invalidServo = structuredClone(handModel)
		const servo = invalidServo.servos[0]
		if (!servo) {
			throw new Error("Expected a servo")
		}
		servo.homeDeg = 100
		expect(() => validateModel(invalidServo)).toThrow("home command must be within its command range")

		servo.homeDeg = 10
		servo.maxTorqueNcm = 0
		expect(() => validateModel(invalidServo)).toThrow("maximum torque must be a positive finite number")
	})
})
