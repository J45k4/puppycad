import { type BodyRef, type FrameRef, type ModelScope, type Point2D, add2, capsule, circle, component, defineModel, directionFromYAxis, roundedRectangle, scale2, v2 } from "../src/model-dsl"

type PalmRefs = {
	backplate: BodyRef
	servoAxes: {
		index: FrameRef
		middle: FrameRef
		thumb: FrameRef
	}
}

type FingerProps = {
	name: string
	groundBody: BodyRef
	driveFrame: FrameRef
	proximalLength: number
	distalLength: number
	distalBendDeg: number
	linkageSide: 1 | -1
	width?: number
}

const shell = { category: "shell", color: "#d6dbe0" }
const servoCase = { category: "servo", color: "#262e38" }
const fingerMaterial = { category: "finger", color: "#384552" }
const linkage = { category: "linkage", color: "#fa6110" }
const joint = { category: "joint", color: "#545f70" }
const pin = { category: "pin", color: "#b8c4d1" }
const pad = { category: "pad", color: "#0e1114" }

const palm = component<undefined, PalmRefs>("Palm and wrist module", (scope) => {
	const backplate = scope.body("backplate", {
		name: "Palm backplate",
		outline: roundedRectangle(v2(0, -13), 98, 94, 9),
		depth: 5,
		appearance: shell
	})
	scope.body("left-rail", {
		name: "Palm left raised rail",
		outline: roundedRectangle(v2(-45, -13), 8, 82, 3),
		depth: 11,
		appearance: shell
	})
	scope.body("right-rail", {
		name: "Palm right raised rail",
		outline: roundedRectangle(v2(45, -15), 8, 72, 3),
		depth: 11,
		appearance: shell
	})
	scope.body("bottom-rail", {
		name: "Palm lower raised rail",
		outline: roundedRectangle(v2(0, -56), 88, 8, 3),
		depth: 11,
		appearance: shell
	})
	scope.body("top-bridge", {
		name: "Palm knuckle bridge",
		outline: roundedRectangle(v2(0, 29), 92, 8, 3),
		depth: 11,
		appearance: shell
	})
	scope.body("wrist-cuff", {
		name: "Wrist mounting cuff",
		outline: roundedRectangle(v2(0, -70), 54, 25, 6),
		depth: 13,
		appearance: shell
	})

	function addServo(id: "index" | "middle" | "thumb", center: Point2D, width: number, height: number, axis: Point2D): FrameRef {
		const housing = scope.body(`servo-${id}`, {
			name: `${title(id)} servo housing`,
			outline: roundedRectangle(center, width, height, 3),
			depth: 15,
			appearance: servoCase
		})
		scope.body(`servo-${id}-cap`, {
			name: `${title(id)} servo top cap`,
			outline: roundedRectangle(center, width - 6, height - 8, 2),
			depth: 16.5,
			appearance: joint
		})
		const groundMount = scope.frame(`servo-${id}-ground-mount`, { body: backplate, at: center })
		const caseMount = scope.frame(`servo-${id}-case-mount`, { body: housing, at: center })
		scope.fixed(`servo-${id}-case-fixed`, {
			name: `${title(id)} servo case to palm`,
			parent: groundMount,
			child: caseMount
		})
		return scope.frame(`servo-${id}-axis`, {
			name: `${title(id)} servo output axis`,
			body: housing,
			at: axis
		})
	}

	const servoAxes = {
		index: addServo("index", v2(-23, 7), 25, 40, v2(-23, 31)),
		middle: addServo("middle", v2(23, 7), 25, 40, v2(23, 31)),
		thumb: addServo("thumb", v2(22, -38), 40, 23, v2(43, -24))
	}

	for (const [index, center] of [v2(-39, -48), v2(39, -48), v2(-39, 23)].entries()) {
		scope.body(`mounting-insert-${index + 1}`, {
			name: `Palm mounting insert ${index + 1}`,
			outline: circle(center, 2.8, 18),
			depth: 12.5,
			appearance: pin
		})
	}

	return { backplate, servoAxes }
})

const finger = component<FingerProps, { tip: FrameRef }>("Single-servo coupled finger", (scope, props) => {
	const width = props.width ?? 17
	const base = v2(0, 0)
	const proximalDirection = v2(0, 1)
	const distalDirection = directionFromYAxis(props.distalBendDeg)
	const pip = add2(base, scale2(proximalDirection, props.proximalLength))
	const tipPoint = add2(pip, scale2(distalDirection, props.distalLength))
	const side = scale2(v2(-proximalDirection.y, proximalDirection.x), props.linkageSide)
	const baseAnchor = add2(add2(base, scale2(side, width * 0.22)), scale2(proximalDirection, 4))
	const distalAnchor = add2(add2(pip, scale2(side, width * 0.22)), scale2(distalDirection, 12))
	const padStart = add2(pip, scale2(distalDirection, Math.max(props.distalLength - 16, 10)))
	const padEnd = add2(pip, scale2(distalDirection, props.distalLength - 2))

	const proximal = scope.body("proximal", {
		name: `${props.name} proximal phalanx`,
		outline: capsule(base, pip, width),
		depth: 11,
		appearance: fingerMaterial
	})
	const distal = scope.body("distal", {
		name: `${props.name} distal phalanx`,
		outline: capsule(pip, tipPoint, width * 0.92),
		depth: 11,
		appearance: fingerMaterial
	})
	scope.body("tip-pad", {
		name: `${props.name} fingertip pad`,
		outline: capsule(padStart, padEnd, width * 0.76),
		depth: 13.5,
		appearance: pad
	})
	scope.body("mcp-joint", {
		name: `${props.name} servo knuckle`,
		outline: circle(base, width * 0.62),
		depth: 13.5,
		appearance: joint
	})
	scope.body("pip-joint", {
		name: `${props.name} distal joint`,
		outline: circle(pip, width * 0.48),
		depth: 13.5,
		appearance: joint
	})
	const groundBracket = scope.body("ground-bracket", {
		name: `${props.name} fixed linkage bracket`,
		outline: capsule(base, baseAnchor, 5.5),
		depth: 14,
		appearance: shell
	})
	const pushrod = scope.body("pushrod", {
		name: `${props.name} rigid four-bar pushrod`,
		outline: capsule(baseAnchor, distalAnchor, 4.5),
		depth: 14.5,
		appearance: linkage
	})
	scope.body("base-pin", {
		name: `${props.name} linkage base pin`,
		outline: circle(baseAnchor, 3.3),
		depth: 17.5,
		appearance: pin
	})
	scope.body("distal-pin", {
		name: `${props.name} linkage distal pin`,
		outline: circle(distalAnchor, 3.3),
		depth: 17.5,
		appearance: pin
	})
	scope.body("mcp-pin", {
		name: `${props.name} servo output pin`,
		outline: circle(base, 3.8),
		depth: 17.5,
		appearance: pin
	})
	scope.body("pip-pin", {
		name: `${props.name} PIP axle pin`,
		outline: circle(pip, 3.2),
		depth: 17,
		appearance: pin
	})

	const proximalMcp = scope.frame("proximal-mcp", { body: proximal, at: base })
	const mcp = scope.revolute("mcp", {
		name: `${props.name} servo joint`,
		parent: props.driveFrame,
		child: proximalMcp,
		limits: { minDeg: -10, maxDeg: 75 }
	})
	scope.servo("drive", {
		name: `${props.name} actuator`,
		joint: mcp,
		homeDeg: 0,
		commandRange: { minDeg: 0, maxDeg: 70 },
		maxTorqueNcm: 25,
		maxSpeedDegPerSec: 300
	})

	const proximalPip = scope.frame("proximal-pip", { body: proximal, at: pip })
	const distalPip = scope.frame("distal-pip", { body: distal, at: pip })
	scope.revolute("pip", {
		name: `${props.name} distal joint`,
		parent: proximalPip,
		child: distalPip,
		limits: { minDeg: -5, maxDeg: 95 }
	})

	const groundBracketMount = scope.frame("ground-bracket-ground", { body: props.groundBody, at: base })
	const bracketMount = scope.frame("ground-bracket-body", { body: groundBracket, at: base })
	scope.fixed("ground-bracket-fixed", {
		name: `${props.name} linkage bracket to palm`,
		parent: groundBracketMount,
		child: bracketMount
	})

	const bracketAnchor = scope.frame("bracket-anchor", { body: groundBracket, at: baseAnchor })
	const pushrodBase = scope.frame("pushrod-base", { body: pushrod, at: baseAnchor })
	scope.revolute("pushrod-base-joint", {
		parent: bracketAnchor,
		child: pushrodBase
	})
	const pushrodDistal = scope.frame("pushrod-distal", { body: pushrod, at: distalAnchor })
	const distalLink = scope.frame("distal-link", { body: distal, at: distalAnchor })
	scope.revolute("pushrod-distal-joint", {
		parent: pushrodDistal,
		child: distalLink
	})

	return {
		tip: scope.frame("tip", { body: distal, at: tipPoint })
	}
})

const model = defineModel(
	{
		id: "three-finger-hand",
		name: "Tendon-free three-servo hand",
		units: "mm",
		metadata: {
			digits: 3,
			servos: 3,
			actuation: "Rigid four-bar distal coupling"
		}
	},
	(hand) => {
		const palmRefs = hand.instance("palm", palm, undefined)
		addFinger(hand, "index", palmRefs, {
			name: "Index",
			axis: palmRefs.servoAxes.index,
			base: v2(-23, 31),
			rotationDeg: 0,
			proximalLength: 50,
			distalLength: 36,
			distalBendDeg: 0,
			linkageSide: 1
		})
		addFinger(hand, "middle", palmRefs, {
			name: "Middle",
			axis: palmRefs.servoAxes.middle,
			base: v2(23, 31),
			rotationDeg: 0,
			proximalLength: 52,
			distalLength: 36,
			distalBendDeg: 0,
			linkageSide: -1
		})
		addFinger(hand, "thumb", palmRefs, {
			name: "Thumb",
			axis: palmRefs.servoAxes.thumb,
			base: v2(43, -24),
			rotationDeg: -56,
			proximalLength: 42,
			distalLength: 31,
			distalBendDeg: 90,
			linkageSide: -1,
			width: 19
		})
	}
)

export default model

type AddFingerOptions = {
	name: string
	axis: FrameRef
	base: Point2D
	rotationDeg: number
	proximalLength: number
	distalLength: number
	distalBendDeg: number
	linkageSide: 1 | -1
	width?: number
}

function addFinger(scope: ModelScope, id: string, palmRefs: PalmRefs, options: AddFingerOptions): void {
	scope.instance(
		id,
		finger,
		{
			name: options.name,
			groundBody: palmRefs.backplate,
			driveFrame: options.axis,
			proximalLength: options.proximalLength,
			distalLength: options.distalLength,
			distalBendDeg: options.distalBendDeg,
			linkageSide: options.linkageSide,
			...(options.width === undefined ? {} : { width: options.width })
		},
		{
			translate: options.base,
			rotateDeg: options.rotationDeg,
			name: `${options.name} finger`
		}
	)
}

function title(value: string): string {
	return `${value.charAt(0).toUpperCase()}${value.slice(1)}`
}
