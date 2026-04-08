export type Point2D = { x: number; y: number }
export type Vector3D = { x: number; y: number; z: number }
export type Point3D = Vector3D
export type Quaternion = { x: number; y: number; z: number; w: number }

export type Transform2D = {
	translation?: Point2D
	rotation?: number
	scale?: Point2D
}

export type Transform3D = {
	translation?: Vector3D
	rotation?: Vector3D
	scale?: Vector3D
}
