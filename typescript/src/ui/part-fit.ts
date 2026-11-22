import { Box3, type Object3D, Mesh, Vector3 } from "three"

/** Measure geometry in content coordinates, independently of the current orbit and pan. */
export function partContentBounds(content: Object3D, solids: Object3D): Box3 {
	content.updateWorldMatrix(true, true)
	const inverse = content.matrixWorld.clone().invert()
	const bounds = new Box3()
	solids.traverse((object) => {
		if (!(object instanceof Mesh)) return
		object.geometry.computeBoundingBox()
		if (object.geometry.boundingBox) bounds.union(object.geometry.boundingBox.clone().applyMatrix4(inverse.clone().multiply(object.matrixWorld)))
	})
	return bounds
}

export function partFitDistance(bounds: Box3, fieldOfView: number, aspect: number): number {
	const vertical = (fieldOfView * Math.PI) / 360
	const horizontal = Math.atan(Math.tan(vertical) * Math.max(0.01, aspect))
	return Math.max(0.5, ((bounds.getSize(new Vector3()).length() * 0.5) / Math.sin(Math.min(vertical, horizontal))) * 1.1)
}
