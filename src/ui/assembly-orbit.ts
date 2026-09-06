import { type OrthographicCamera, Vector3 } from "three"
/** Right drag rotates; middle drag pans; primary touch/pen drag rotates. */
export function bindAssemblyOrbit(
	viewport: HTMLElement,
	rotate: (dx: number, dy: number) => void,
	pan: (dx: number, dy: number) => void,
	beginRotate?: (clientX: number, clientY: number) => void
): void {
	let pointer: { id: number; x: number; y: number; mouse: boolean; pan: boolean } | null = null
	const stop = () => {
		pointer = null
		viewport.style.cursor = "grab"
	}
	viewport.style.cursor = "grab"
	viewport.oncontextmenu = (event) => event.preventDefault()
	viewport.onpointerdown = (event) => {
		const mouse = event.pointerType === "mouse"
		if (mouse ? event.button !== 2 && event.button !== 1 : !event.isPrimary || event.button !== 0) return
		event.preventDefault()
		pointer = { id: event.pointerId, x: event.clientX, y: event.clientY, mouse, pan: mouse && event.button === 1 }
		if (!pointer.pan) beginRotate?.(event.clientX, event.clientY)
		viewport.setPointerCapture(event.pointerId)
		viewport.style.cursor = "grabbing"
	}
	viewport.onpointermove = (event) => {
		if (!pointer || event.pointerId !== pointer.id) return
		if (pointer.mouse && (event.buttons & (pointer.pan ? 4 : 2)) === 0) {
			stop()
			return
		}
		event.preventDefault()
		const move = pointer.pan ? pan : rotate
		move(event.clientX - pointer.x, event.clientY - pointer.y)
		pointer.x = event.clientX
		pointer.y = event.clientY
	}
	viewport.onpointerup =
		viewport.onpointercancel =
		viewport.onlostpointercapture =
			(event) => {
				if (pointer?.id === event.pointerId) stop()
			}
}

/** Move the view target in screen axes, with pixel-consistent drag distance at any zoom. */
export function panAssemblyTarget(camera: OrthographicCamera, target: Vector3, dx: number, dy: number, viewportHeight: number): void {
	camera.updateMatrixWorld()
	const unitsPerPixel = (camera.top - camera.bottom) / (camera.zoom * Math.max(1, viewportHeight))
	target.addScaledVector(new Vector3().setFromMatrixColumn(camera.matrixWorld, 0), -dx * unitsPerPixel)
	target.addScaledVector(new Vector3().setFromMatrixColumn(camera.matrixWorld, 1), dy * unitsPerPixel)
}
