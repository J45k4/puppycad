import { Buffer } from "node:buffer"
import { spawn } from "node:child_process"
import zlib from "node:zlib"
import type { SolidEdge, SolidFace, SolidVertex } from "./schema"
import type { Vector3D } from "./types"

export type RenderGeometryBody = {
	readonly id: string
	readonly partId: string
	readonly sourceId: string
	readonly vertices: readonly SolidVertex[]
	readonly edges: readonly SolidEdge[]
	readonly faces: readonly SolidFace[]
}

export type RenderPreviewOptions = {
	width?: number
	height?: number
	background?: [number, number, number, number]
	createContext?: HeadlessGlFactory
}

type HeadlessGlFactory = (width: number, height: number, options?: Record<string, unknown>) => WebGLRenderingContextLike | null

type WebGLRenderingContextLike = WebGLRenderingContext & {
	readonly drawingBufferWidth?: number
	readonly drawingBufferHeight?: number
}

type Vec3 = [number, number, number]
type Mat4 = readonly number[]

const DEFAULT_WIDTH = 1024
const DEFAULT_HEIGHT = 768
const DEFAULT_BACKGROUND: [number, number, number, number] = [0.93, 0.96, 0.99, 1]

export async function renderProjectPreviewPng(bodies: readonly RenderGeometryBody[], options: RenderPreviewOptions = {}): Promise<Uint8Array> {
	const width = normalizeImageDimension(options.width, DEFAULT_WIDTH, "width")
	const height = normalizeImageDimension(options.height, DEFAULT_HEIGHT, "height")
	const background = options.background ?? DEFAULT_BACKGROUND
	const mesh = buildPreviewMesh(bodies)
	if (mesh.positions.length === 0) {
		throw new Error("Project has no generated solid geometry to render.")
	}
	if (!options.createContext) {
		return renderMeshWithNodeWorker(mesh, width, height, background)
	}
	const createContext = options.createContext
	const gl = createContext(width, height, { preserveDrawingBuffer: true, antialias: true })
	if (!gl) {
		throw new Error("Headless rendering failed to create a WebGL context. On Linux, try running through xvfb-run and ensure Mesa/X11 libraries are installed.")
	}

	const program = createProgram(gl, VERTEX_SHADER_SOURCE, FRAGMENT_SHADER_SOURCE)
	gl.useProgram(program)
	gl.viewport(0, 0, width, height)
	gl.clearColor(background[0], background[1], background[2], background[3])
	gl.clearDepth(1)
	gl.enable(gl.DEPTH_TEST)
	gl.enable(gl.CULL_FACE)
	gl.cullFace(gl.BACK)
	gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT)

	const modelViewProjection = createPreviewMatrix(mesh.bounds, width / height)
	const normalMatrix = createNormalMatrix(mesh.modelMatrix)

	bindAttribute(gl, program, "position", mesh.positions, 3)
	bindAttribute(gl, program, "normal", mesh.normals, 3)
	gl.uniformMatrix4fv(gl.getUniformLocation(program, "modelViewProjection"), false, new Float32Array(modelViewProjection))
	gl.uniformMatrix4fv(gl.getUniformLocation(program, "model"), false, new Float32Array(mesh.modelMatrix))
	gl.uniformMatrix4fv(gl.getUniformLocation(program, "normalMatrix"), false, new Float32Array(normalMatrix))
	gl.uniform3f(gl.getUniformLocation(program, "baseColor"), 0.16, 0.55, 0.84)
	gl.uniform3f(gl.getUniformLocation(program, "lightDirection"), 0.45, 0.75, 0.48)
	gl.drawArrays(gl.TRIANGLES, 0, mesh.positions.length / 3)

	return readPngFromGl(gl, width, height)
}

function buildPreviewMesh(bodies: readonly RenderGeometryBody[]): { positions: number[]; normals: number[]; bounds: Bounds; modelMatrix: Mat4 } {
	const positions: number[] = []
	const normals: number[] = []
	const bounds = createEmptyBounds()
	for (const body of bodies) {
		const verticesById = new Map(body.vertices.map((vertex) => [vertex.id, vertex.position] as const))
		const edgesById = new Map(body.edges.map((edge) => [edge.id, edge] as const))
		for (const vertex of body.vertices) {
			expandBounds(bounds, toVec3(vertex.position))
		}
		for (const face of body.faces) {
			const polygon = orderFaceVertices(face, edgesById, verticesById)
			if (polygon.length < 3) {
				continue
			}
			const normal = computeFaceNormal(polygon)
			const origin = polygon[0]
			if (!origin) {
				continue
			}
			for (let index = 1; index < polygon.length - 1; index += 1) {
				const current = polygon[index]
				const next = polygon[index + 1]
				if (!current || !next) {
					continue
				}
				pushVertex(positions, normals, origin, normal)
				pushVertex(positions, normals, current, normal)
				pushVertex(positions, normals, next, normal)
			}
		}
	}
	return { positions, normals, bounds, modelMatrix: identity4() }
}

async function renderMeshWithNodeWorker(
	mesh: { positions: number[]; normals: number[]; bounds: Bounds; modelMatrix: Mat4 },
	width: number,
	height: number,
	background: [number, number, number, number]
): Promise<Uint8Array> {
	const input = JSON.stringify({ width, height, background, mesh, vertexShader: VERTEX_SHADER_SOURCE, fragmentShader: FRAGMENT_SHADER_SOURCE })
	const child = spawn(process.env.PUPPYCAD_NODE_RENDERER ?? "node", ["-e", NODE_RENDER_WORKER], {
		stdio: ["pipe", "pipe", "pipe"],
		env: process.env
	})
	const stdout: Buffer[] = []
	const stderr: Buffer[] = []
	child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk))
	child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk))
	child.stdin.end(input)
	const code = await new Promise<number | null>((resolve, reject) => {
		child.on("error", reject)
		child.on("close", resolve)
	})
	if (code !== 0) {
		throw new Error(`Headless rendering failed in node worker. ${Buffer.concat(stderr).toString("utf8").trim()}`.trim())
	}
	const payload = JSON.parse(Buffer.concat(stdout).toString("utf8")) as { pngBase64?: unknown; error?: unknown }
	if (typeof payload.error === "string") {
		throw new Error(payload.error)
	}
	if (typeof payload.pngBase64 !== "string") {
		throw new Error("Headless rendering worker did not return PNG data.")
	}
	return Buffer.from(payload.pngBase64, "base64")
}

const NODE_RENDER_WORKER = String.raw`
const { createRequire } = require('node:module')
const zlib = require('node:zlib')
const requireFromCwd = createRequire(process.cwd() + '/package.json')
let raw = ''
process.stdin.setEncoding('utf8')
process.stdin.on('data', (chunk) => { raw += chunk })
process.stdin.on('end', () => {
  try {
    const input = JSON.parse(raw)
    const createGL = requireFromCwd('gl')
    const gl = createGL(input.width, input.height, { preserveDrawingBuffer: true, antialias: true })
    if (!gl) throw new Error('Headless rendering failed to create a WebGL context. On Linux, run the command through xvfb-run and ensure Mesa/X11 libraries are installed.')
    const program = createProgram(gl, input.vertexShader, input.fragmentShader)
    gl.useProgram(program)
    const bg = input.background
    gl.viewport(0, 0, input.width, input.height)
    gl.clearColor(bg[0], bg[1], bg[2], bg[3])
    gl.clearDepth(1)
    gl.enable(gl.DEPTH_TEST)
    gl.enable(gl.CULL_FACE)
    gl.cullFace(gl.BACK)
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT)
    const mvp = createPreviewMatrix(input.mesh.bounds, input.width / input.height)
    bindAttribute(gl, program, 'position', input.mesh.positions, 3)
    bindAttribute(gl, program, 'normal', input.mesh.normals, 3)
    gl.uniformMatrix4fv(gl.getUniformLocation(program, 'modelViewProjection'), false, new Float32Array(mvp))
    gl.uniformMatrix4fv(gl.getUniformLocation(program, 'model'), false, new Float32Array(input.mesh.modelMatrix))
    gl.uniformMatrix4fv(gl.getUniformLocation(program, 'normalMatrix'), false, new Float32Array(input.mesh.modelMatrix))
    gl.uniform3f(gl.getUniformLocation(program, 'baseColor'), 0.16, 0.55, 0.84)
    gl.uniform3f(gl.getUniformLocation(program, 'lightDirection'), 0.45, 0.75, 0.48)
    gl.drawArrays(gl.TRIANGLES, 0, input.mesh.positions.length / 3)
    process.stdout.write(JSON.stringify({ pngBase64: readPngFromGl(gl, input.width, input.height).toString('base64') }))
  } catch (error) {
    process.stdout.write(JSON.stringify({ error: error && error.message ? error.message : String(error) }))
  }
})

function bindAttribute(gl, program, name, values, size) {
  const location = gl.getAttribLocation(program, name)
  if (location < 0) return
  const buffer = gl.createBuffer()
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(values), gl.STATIC_DRAW)
  gl.enableVertexAttribArray(location)
  gl.vertexAttribPointer(location, size, gl.FLOAT, false, 0, 0)
}
function createProgram(gl, vertexSource, fragmentSource) {
  const program = gl.createProgram()
  gl.attachShader(program, createShader(gl, gl.VERTEX_SHADER, vertexSource))
  gl.attachShader(program, createShader(gl, gl.FRAGMENT_SHADER, fragmentSource))
  gl.linkProgram(program)
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) || 'Unable to link WebGL program.')
  return program
}
function createShader(gl, type, source) {
  const shader = gl.createShader(type)
  gl.shaderSource(shader, source)
  gl.compileShader(shader)
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) || 'Unable to compile WebGL shader.')
  return shader
}
function createPreviewMatrix(bounds, aspect) {
  const center = [(bounds.min[0] + bounds.max[0]) / 2, (bounds.min[1] + bounds.max[1]) / 2, (bounds.min[2] + bounds.max[2]) / 2]
  const size = Math.max(bounds.max[0] - bounds.min[0], bounds.max[1] - bounds.min[1], bounds.max[2] - bounds.min[2], 1)
  const eye = [center[0] + size * 1.7, center[1] + size * 1.25, center[2] + size * 1.9]
  return multiply4(perspective(35 * Math.PI / 180, aspect, 0.1, size * 8), lookAt(eye, center, [0, 1, 0]))
}
function perspective(fovy, aspect, near, far) {
  const f = 1 / Math.tan(fovy / 2)
  const nf = 1 / (near - far)
  return [f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]
}
function lookAt(eye, target, up) {
  const z = normalize3(sub3(eye, target))
  const x = normalize3(cross3(up, z))
  const y = cross3(z, x)
  return [x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, -dot3(x, eye), -dot3(y, eye), -dot3(z, eye), 1]
}
function multiply4(a, b) {
  const out = new Array(16).fill(0)
  for (let row = 0; row < 4; row++) for (let col = 0; col < 4; col++) out[col * 4 + row] = a[0 * 4 + row] * b[col * 4 + 0] + a[1 * 4 + row] * b[col * 4 + 1] + a[2 * 4 + row] * b[col * 4 + 2] + a[3 * 4 + row] * b[col * 4 + 3]
  return out
}
function readPngFromGl(gl, width, height) {
  const pixels = new Uint8Array(width * height * 4)
  gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, pixels)
  const raw = Buffer.alloc((width * 4 + 1) * height)
  for (let y = 0; y < height; y++) {
    const sy = height - 1 - y
    const row = y * (width * 4 + 1)
    raw[row] = 0
    for (let x = 0; x < width; x++) {
      const source = (sy * width + x) * 4
      const target = row + 1 + x * 4
      raw[target] = pixels[source]
      raw[target + 1] = pixels[source + 1]
      raw[target + 2] = pixels[source + 2]
      raw[target + 3] = pixels[source + 3]
    }
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 6
  return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]), chunk('IHDR', header), chunk('IDAT', zlib.deflateSync(raw, { level: 9 })), chunk('IEND', Buffer.alloc(0))])
}
function chunk(type, data) {
  const name = Buffer.from(type)
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length)
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(Buffer.concat([name, data])))
  return Buffer.concat([len, name, data, crc])
}
function crc32(buffer) {
  let checksum = ~0
  for (let i = 0; i < buffer.length; i++) { checksum ^= buffer[i]; for (let bit = 0; bit < 8; bit++) checksum = (checksum >>> 1) ^ (0xedb88320 & -(checksum & 1)) }
  return ~checksum >>> 0
}
function sub3(a, b) { return [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
function cross3(a, b) { return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]] }
function dot3(a, b) { return a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
function normalize3(v) { const l = Math.hypot(v[0], v[1], v[2]) || 1; return [v[0]/l, v[1]/l, v[2]/l] }
`

function orderFaceVertices(face: SolidFace, edgesById: ReadonlyMap<string, SolidEdge>, verticesById: ReadonlyMap<string, Vector3D>): Vec3[] {
	const edges = face.edgeIds.map((edgeId) => edgesById.get(edgeId)).filter((edge): edge is SolidEdge => !!edge && edge.vertexIds.length >= 2)
	const first = edges[0]
	if (!first) {
		return []
	}
	const start = first.vertexIds[0]
	const second = first.vertexIds[1]
	if (!start || !second) {
		return []
	}
	const orderedIds = [start, second]
	const unused = edges.slice(1)
	let current = second
	while (unused.length > 0 && current !== start) {
		const nextIndex = unused.findIndex((edge) => edge.vertexIds.includes(current))
		if (nextIndex < 0) {
			break
		}
		const [edge] = unused.splice(nextIndex, 1)
		const next = edge?.vertexIds.find((vertexId) => vertexId !== current)
		if (!next) {
			break
		}
		if (next !== start) {
			orderedIds.push(next)
		}
		current = next
	}
	return orderedIds
		.map((vertexId) => verticesById.get(vertexId))
		.filter((position): position is Vector3D => !!position)
		.map(toVec3)
}

function pushVertex(positions: number[], normals: number[], position: Vec3, normal: Vec3): void {
	positions.push(position[0], position[1], position[2])
	normals.push(normal[0], normal[1], normal[2])
}

function computeFaceNormal(points: readonly Vec3[]): Vec3 {
	const a = points[0]
	const b = points[1]
	const c = points[2]
	if (!a || !b || !c) {
		return [0, 1, 0]
	}
	return normalize3(cross3(sub3(b, a), sub3(c, a)))
}

function bindAttribute(gl: WebGLRenderingContext, program: WebGLProgram, name: string, values: readonly number[], size: number): void {
	const location = gl.getAttribLocation(program, name)
	if (location < 0) {
		return
	}
	const buffer = gl.createBuffer()
	gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
	gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(values), gl.STATIC_DRAW)
	gl.enableVertexAttribArray(location)
	gl.vertexAttribPointer(location, size, gl.FLOAT, false, 0, 0)
}

function createProgram(gl: WebGLRenderingContext, vertexSource: string, fragmentSource: string): WebGLProgram {
	const vertexShader = createShader(gl, gl.VERTEX_SHADER, vertexSource)
	const fragmentShader = createShader(gl, gl.FRAGMENT_SHADER, fragmentSource)
	const program = gl.createProgram()
	if (!program) {
		throw new Error("Unable to create WebGL program.")
	}
	gl.attachShader(program, vertexShader)
	gl.attachShader(program, fragmentShader)
	gl.linkProgram(program)
	if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
		throw new Error(gl.getProgramInfoLog(program) ?? "Unable to link WebGL program.")
	}
	return program
}

function createShader(gl: WebGLRenderingContext, type: number, source: string): WebGLShader {
	const shader = gl.createShader(type)
	if (!shader) {
		throw new Error("Unable to create WebGL shader.")
	}
	gl.shaderSource(shader, source)
	gl.compileShader(shader)
	if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
		throw new Error(gl.getShaderInfoLog(shader) ?? "Unable to compile WebGL shader.")
	}
	return shader
}

function createPreviewMatrix(bounds: Bounds, aspect: number): Mat4 {
	const center = boundsCenter(bounds)
	const size = Math.max(bounds.max[0] - bounds.min[0], bounds.max[1] - bounds.min[1], bounds.max[2] - bounds.min[2], 1)
	const eye = add3(center, [size * 1.7, size * 1.25, size * 1.9])
	const view = lookAt(eye, center, [0, 1, 0])
	const projection = perspective((35 * Math.PI) / 180, aspect, 0.1, size * 8)
	return multiply4(projection, view)
}

function createNormalMatrix(model: Mat4): Mat4 {
	return model
}

function readPngFromGl(gl: WebGLRenderingContext, width: number, height: number): Uint8Array {
	const pixels = new Uint8Array(width * height * 4)
	gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, pixels)
	const raw = Buffer.alloc((width * 4 + 1) * height)
	for (let y = 0; y < height; y += 1) {
		const sourceY = height - 1 - y
		const rowStart = y * (width * 4 + 1)
		raw[rowStart] = 0
		for (let x = 0; x < width; x += 1) {
			const source = (sourceY * width + x) * 4
			const target = rowStart + 1 + x * 4
			raw[target] = pixels[source] ?? 0
			raw[target + 1] = pixels[source + 1] ?? 0
			raw[target + 2] = pixels[source + 2] ?? 0
			raw[target + 3] = pixels[source + 3] ?? 255
		}
	}
	const header = Buffer.alloc(13)
	header.writeUInt32BE(width, 0)
	header.writeUInt32BE(height, 4)
	header[8] = 8
	header[9] = 6
	return Buffer.concat([PNG_SIGNATURE, pngChunk("IHDR", header), pngChunk("IDAT", zlib.deflateSync(raw, { level: 9 })), pngChunk("IEND", Buffer.alloc(0))])
}

function pngChunk(type: string, data: Buffer): Buffer {
	const name = Buffer.from(type)
	const length = Buffer.alloc(4)
	length.writeUInt32BE(data.length)
	const checksum = Buffer.alloc(4)
	checksum.writeUInt32BE(crc32(Buffer.concat([name, data])))
	return Buffer.concat([length, name, data, checksum])
}

function crc32(buffer: Buffer): number {
	let checksum = ~0
	for (let index = 0; index < buffer.length; index += 1) {
		checksum ^= buffer[index] ?? 0
		for (let bit = 0; bit < 8; bit += 1) {
			checksum = (checksum >>> 1) ^ (0xedb88320 & -(checksum & 1))
		}
	}
	return ~checksum >>> 0
}

type Bounds = {
	min: Vec3
	max: Vec3
}

function createEmptyBounds(): Bounds {
	return {
		min: [Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY],
		max: [Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY]
	}
}

function expandBounds(bounds: Bounds, point: Vec3): void {
	bounds.min = [Math.min(bounds.min[0], point[0]), Math.min(bounds.min[1], point[1]), Math.min(bounds.min[2], point[2])]
	bounds.max = [Math.max(bounds.max[0], point[0]), Math.max(bounds.max[1], point[1]), Math.max(bounds.max[2], point[2])]
}

function boundsCenter(bounds: Bounds): Vec3 {
	return [(bounds.min[0] + bounds.max[0]) / 2, (bounds.min[1] + bounds.max[1]) / 2, (bounds.min[2] + bounds.max[2]) / 2]
}

function perspective(fovy: number, aspect: number, near: number, far: number): Mat4 {
	const f = 1 / Math.tan(fovy / 2)
	const nf = 1 / (near - far)
	return [f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]
}

function lookAt(eye: Vec3, target: Vec3, up: Vec3): Mat4 {
	const z = normalize3(sub3(eye, target))
	const x = normalize3(cross3(up, z))
	const y = cross3(z, x)
	return [x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, -dot3(x, eye), -dot3(y, eye), -dot3(z, eye), 1]
}

function multiply4(a: Mat4, b: Mat4): Mat4 {
	const out = new Array<number>(16).fill(0)
	for (let row = 0; row < 4; row += 1) {
		for (let column = 0; column < 4; column += 1) {
			out[column * 4 + row] =
				(a[0 * 4 + row] ?? 0) * (b[column * 4 + 0] ?? 0) +
				(a[1 * 4 + row] ?? 0) * (b[column * 4 + 1] ?? 0) +
				(a[2 * 4 + row] ?? 0) * (b[column * 4 + 2] ?? 0) +
				(a[3 * 4 + row] ?? 0) * (b[column * 4 + 3] ?? 0)
		}
	}
	return out
}

function identity4(): Mat4 {
	return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
}

function toVec3(vector: Vector3D): Vec3 {
	return [vector.x, vector.y, vector.z]
}

function add3(a: Vec3, b: Vec3): Vec3 {
	return [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

function sub3(a: Vec3, b: Vec3): Vec3 {
	return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

function cross3(a: Vec3, b: Vec3): Vec3 {
	return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

function dot3(a: Vec3, b: Vec3): number {
	return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

function normalize3(vector: Vec3): Vec3 {
	const length = Math.hypot(vector[0], vector[1], vector[2]) || 1
	return [vector[0] / length, vector[1] / length, vector[2] / length]
}

function normalizeImageDimension(value: number | undefined, fallback: number, label: string): number {
	const resolved = value ?? fallback
	if (!Number.isInteger(resolved) || resolved < 16 || resolved > 4096) {
		throw new Error(`Render ${label} must be an integer between 16 and 4096.`)
	}
	return resolved
}

function formatError(error: unknown): string {
	return error instanceof Error ? error.message : String(error)
}

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])

const VERTEX_SHADER_SOURCE = `
attribute vec3 position;
attribute vec3 normal;
uniform mat4 modelViewProjection;
uniform mat4 model;
uniform mat4 normalMatrix;
varying vec3 vNormal;
varying vec3 vWorldPosition;
void main() {
	vec4 worldPosition = model * vec4(position, 1.0);
	vWorldPosition = worldPosition.xyz;
	vNormal = normalize((normalMatrix * vec4(normal, 0.0)).xyz);
	gl_Position = modelViewProjection * vec4(position, 1.0);
}
`

const FRAGMENT_SHADER_SOURCE = `
precision mediump float;
uniform vec3 baseColor;
uniform vec3 lightDirection;
varying vec3 vNormal;
varying vec3 vWorldPosition;
void main() {
	vec3 normal = normalize(vNormal);
	vec3 light = normalize(lightDirection);
	float diffuse = max(dot(normal, light), 0.0);
	float rim = pow(1.0 - abs(normal.z), 2.0) * 0.16;
	vec3 color = baseColor * (0.35 + diffuse * 0.72) + vec3(0.75, 0.9, 1.0) * rim;
	gl_FragColor = vec4(pow(color, vec3(1.0 / 2.2)), 1.0);
}
`
