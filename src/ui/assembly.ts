import { pickOrbitPivot, orbitAssemblyTarget } from "./orbit-pivot"
import { bindAssemblyOrbit, panAssemblyTarget } from "./assembly-orbit"
import { AssemblyProperties } from "./assembly-properties"
import { requireValue } from "../required"
import { AmbientLight, Box3, Color, DirectionalLight, Group, Mesh, MeshLambertMaterial, OrthographicCamera, Vector2, Scene, Vector3, WebGLRenderer } from "three"
import type { Assembly, ProjectNode, ProjectPartDocument } from "../contract"
import { createPartGeometries, exportPartStl } from "../part-mesh"
import { transformMatrix, solveFixedAssembly } from "../assembly-solver"
import { UiComponent } from "./ui"
const colors = [0x65b5a4, 0xe3b365, 0x8b9fe0, 0xd38d9e, 0xa2c27d]
export class AssemblyEditor extends UiComponent<HTMLDivElement> {
	private assembly: Assembly
	private refreshPreview = () => {}
	public refreshParts(): void {
		this.refreshPreview()
	}
	constructor(args?: {
		assembly?: Assembly
		getParts?: () => ProjectNode[]
		onChange?: (next: Assembly, previous: Assembly) => void
		onOpenPart?: (id: string) => void
	}) {
		super(document.createElement("div"))
		this.assembly = solveFixedAssembly(args?.assembly ?? { id: "assembly", name: "Assembly", instances: [] })
		this.root.style.cssText = "width:100%;height:100%;display:flex;flex-direction:column;background:#edf2f5;color:#263544;overflow:auto"
		const heading = document.createElement("div")
		heading.style.cssText = "padding:16px;display:flex;gap:16px;align-items:center;flex-wrap:wrap"
		const title = document.createElement("strong")
		title.textContent = `${this.assembly.name} · ${this.assembly.instances.length} instances · ${this.assembly.mates?.length ?? 0} connections`
		const hint = document.createElement("span")
		hint.textContent = "Right-drag to rotate · Middle-drag to pan · Scroll to zoom · Dimensions in mm"
		heading.append(title, hint)
		this.root.append(heading)
		const viewport = document.createElement("div")
		viewport.style.cssText = "flex:1;min-height:300px;overflow:hidden;touch-action:none;position:relative"
		viewport.tabIndex = 0
		viewport.setAttribute("aria-label", "Assembly preview. Hold the right mouse button and drag to rotate. Hold the middle button and drag to pan. Arrow keys rotate; plus and minus zoom.")
		const workspace = document.createElement("div")
		workspace.style.cssText = "display:flex;flex:1;min-height:0"
		const properties = new AssemblyProperties(
			this.assembly,
			args?.getParts ?? (() => []),
			(next) => {
				args?.onChange?.(next, this.getState())
				this.assembly = next
				title.textContent = `${next.name} · ${next.instances.length} instances · ${next.mates?.length ?? 0} connections`
				this.refreshPreview()
			},
			args?.onOpenPart
		)
		workspace.append(properties.root, viewport)
		this.root.append(workspace)
		const footer = document.createElement("div")
		footer.style.cssText = "display:flex;gap:8px;flex-wrap:wrap;padding:12px"
		this.root.append(footer)
		let initialized = false
		let render = () => {}
		let disposeRenderer = () => {}
		const initialize = () => {
			if (initialized || !this.root.isConnected) return
			initialized = true
			properties.refreshChoices()
			const parts = new Map<string, ProjectPartDocument>()
			const visit = (nodes: ProjectNode[]) => {
				for (const node of nodes) {
					if ("kind" in node) visit(node.items)
					else if (node.type === "part") parts.set(node.id, node)
				}
			}
			visit(args?.getParts?.() ?? [])
			const scene = new Scene()
			scene.background = new Color(0xedf2f5)
			scene.add(new AmbientLight(0xffffff, 1.8))
			const light = new DirectionalLight(0xffffff, 2.5)
			light.position.set(100, -100, 200)
			scene.add(light)
			const group = new Group()
			scene.add(group)
			const partColors = new Map<string, number>()
			const errors: string[] = []
			for (const instance of this.assembly.instances) {
				const part = parts.get(instance.partId)
				if (!part?.data) {
					errors.push(`Missing part: ${instance.partId}`)
					continue
				}
				try {
					if (!partColors.has(part.id)) partColors.set(part.id, requireValue(colors[partColors.size % colors.length]))
					const material = new MeshLambertMaterial({ color: partColors.get(part.id) })
					for (const geometry of createPartGeometries(part.data)) {
						geometry.applyMatrix4(transformMatrix(instance.transform))
						group.add(new Mesh(geometry, material))
					}
				} catch (error) {
					errors.push(`${part.name}: ${error instanceof Error ? error.message : error}`)
				}
			}
			for (const partId of partColors.keys()) {
				const part = requireValue(parts.get(partId))
				const button = document.createElement("button")
				button.textContent = `Download ${part.name}.stl`
				button.onclick = () => {
					try {
						const url = URL.createObjectURL(new Blob([exportPartStl(requireValue(part.data))], { type: "model/stl" }))
						const link = document.createElement("a")
						link.href = url
						link.download = `${part.id.replace(/[^a-zA-Z0-9._-]/g, "-")}.stl`
						link.click()
						setTimeout(() => URL.revokeObjectURL(url), 1000)
					} catch (error) {
						window.alert(error instanceof Error ? error.message : String(error))
					}
				}
				footer.append(button)
			}
			if (errors.length) {
				const error = document.createElement("p")
				error.textContent = errors.join("; ")
				error.setAttribute("role", "alert")
				footer.append(error)
			}
			const bounds = new Box3().setFromObject(group)
			const center = bounds.isEmpty() ? new Vector3() : bounds.getCenter(new Vector3())
			const size = bounds.isEmpty() ? 100 : Math.max(bounds.getSize(new Vector3()).length(), 1)
			let pivot = center.clone()
			let yaw = -Math.PI / 4
			let pitch = Math.PI / 5
			let zoom = 1
			const camera = new OrthographicCamera(-size, size, size, -size, 0.01, size * 20)
			camera.up.set(0, 0, 1)
			const renderer = new WebGLRenderer({ antialias: true })
			renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
			renderer.domElement.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block"
			disposeRenderer = () => {
				for (const child of group.children) {
					if (child instanceof Mesh) {
						child.geometry.dispose()
						;(child.material as MeshLambertMaterial).dispose()
					}
				}
				renderer.dispose()
				renderer.forceContextLoss()
			}
			viewport.append(renderer.domElement)
			render = () => {
				const width = viewport.clientWidth || 800
				const height = viewport.clientHeight || 500
				const halfHeight = (size * 0.55) / zoom
				const aspect = width / height
				camera.left = -halfHeight * aspect
				camera.right = halfHeight * aspect
				camera.top = halfHeight
				camera.bottom = -halfHeight
				camera.position.set(center.x + Math.cos(yaw) * Math.cos(pitch) * size * 3, center.y + Math.sin(yaw) * Math.cos(pitch) * size * 3, center.z + Math.sin(pitch) * size * 3)
				camera.lookAt(center)
				camera.updateProjectionMatrix()
				renderer.setSize(width, height)
				renderer.render(scene, camera)
			}
			bindAssemblyOrbit(
				viewport,
				(dx, dy) => {
					const nextYaw = yaw - dx * 0.01
					const nextPitch = Math.max(-1.5, Math.min(1.5, pitch + dy * 0.01))
					orbitAssemblyTarget(center, pivot, yaw, pitch, nextYaw, nextPitch)
					yaw = nextYaw
					pitch = nextPitch
					render()
				},
				(dx, dy) => {
					panAssemblyTarget(camera, center, dx, dy, viewport.clientHeight || 500)
					render()
				},
				(clientX, clientY) => {
					const rect = viewport.getBoundingClientRect()
					const pointer = new Vector2(((clientX - rect.left) / Math.max(1, rect.width)) * 2 - 1, 1 - ((clientY - rect.top) / Math.max(1, rect.height)) * 2)
					pivot = pickOrbitPivot(camera, pointer, [group], size * 3)
				}
			)
			viewport.onwheel = (event) => {
				event.preventDefault()
				zoom = Math.max(0.2, Math.min(10, zoom * Math.exp(-event.deltaY * 0.001)))
				render()
			}
			viewport.onkeydown = (event) => {
				if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "+", "-"].includes(event.key)) return
				event.preventDefault()
				if (event.key === "ArrowLeft") yaw -= 0.1
				if (event.key === "ArrowRight") yaw += 0.1
				if (event.key === "ArrowUp") pitch = Math.min(1.5, pitch + 0.1)
				if (event.key === "ArrowDown") pitch = Math.max(-1.5, pitch - 0.1)
				if (event.key === "+") zoom = Math.min(10, zoom * 1.1)
				if (event.key === "-") zoom = Math.max(0.2, zoom / 1.1)
				render()
			}
			render()
		}
		this.refreshPreview = () => {
			disposeRenderer()
			initialized = false
			viewport.replaceChildren()
			footer.replaceChildren()
			initialize()
		}
		if (typeof ResizeObserver !== "undefined") {
			let frame: number | null = null
			const observer = new ResizeObserver(() => {
				if (frame !== null) cancelAnimationFrame(frame)
				frame = requestAnimationFrame(() => {
					frame = null
					initialize()
					render()
				})
			})
			observer.observe(viewport)
			this.dispose = () => {
				observer.disconnect()
				if (frame !== null) cancelAnimationFrame(frame)
				disposeRenderer()
			}
		}
		if (typeof ResizeObserver === "undefined") this.dispose = () => disposeRenderer()
		queueMicrotask(initialize)
	}
	getState(): Assembly {
		return structuredClone(this.assembly)
	}
	dispose(): void {}
}
