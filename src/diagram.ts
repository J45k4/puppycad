import { CanvasComponent, Connection, EditorCanvas } from "./canvas"
import { UiComponent, VList, showTextPromptModal } from "./ui"

type FlowchartShape =
        | "startEnd"
        | "process"
        | "decision"
        | "inputOutput"
        | "predefinedProcess"
        | "manualInput"
        | "document"
        | "database"
        | "entity"

type ERAttribute = {
        name: string
        type: string
        isPrimaryKey?: boolean
        isForeignKey?: boolean
        isUnique?: boolean
        allowNull?: boolean
}

type DiagramNodeData = {
        shape: FlowchartShape
        text: string
        erAttributes?: ERAttribute[]
}

type DiagramComponent = CanvasComponent<DiagramNodeData>

type PersistedDiagramState = {
        components: DiagramComponent[]
        connections: Connection[]
}

type ShapeConfig = {
        label: string
        width: number
        height: number
        defaultText: string
        textColor?: string
}

const SHAPE_CONFIG: Record<FlowchartShape, ShapeConfig> = {
        startEnd: { label: "Start / End", width: 140, height: 60, defaultText: "Start" },
        process: { label: "Process", width: 160, height: 80, defaultText: "Process" },
        decision: { label: "Decision", width: 160, height: 160, defaultText: "Decision" },
        inputOutput: { label: "Input / Output", width: 170, height: 80, defaultText: "Data" },
        predefinedProcess: { label: "Subprocess", width: 180, height: 80, defaultText: "Subprocess" },
        manualInput: { label: "Manual Input", width: 180, height: 80, defaultText: "Manual Input" },
        document: { label: "Document", width: 200, height: 120, defaultText: "Document" },
        database: { label: "Database", width: 160, height: 110, defaultText: "Database" },
        entity: { label: "Entity", width: 200, height: 140, defaultText: "Entity", textColor: "#78350f" }
}

const FLOWCHART_SHAPES: FlowchartShape[] = [
        "startEnd",
        "process",
        "decision",
        "inputOutput",
        "predefinedProcess",
        "manualInput",
        "document",
        "database"
]

const ER_MODEL_SHAPES: FlowchartShape[] = ["entity"]

const STORAGE_KEY = "puppycad-diagram-state"

const entityAttributeDescription =
        "Enter one attribute per line using `name:type` syntax. Optionally append `!` for required fields or `#` for primary keys."

export class DiagramEditor extends UiComponent<HTMLDivElement> {
        private readonly palette: VList
        private readonly editor: EditorCanvas<DiagramNodeData>
        private persistHandle: number | null = null

        public constructor() {
                super(document.createElement("div"))
                this.root.style.display = "flex"
                this.root.style.gap = "16px"
                this.root.style.height = "100%"
                this.root.style.boxSizing = "border-box"

                this.palette = new VList({
                        style: {
                                width: "240px",
                                gap: "12px",
                                padding: "16px",
                                background: "#f1f5f9",
                                borderRadius: "16px",
                                boxSizing: "border-box",
                                overflowY: "auto",
                                maxHeight: "100%"
                        }
                })
                this.root.appendChild(this.palette.root)

                const canvasContainer = document.createElement("div")
                canvasContainer.style.flex = "1 1 auto"
                canvasContainer.style.display = "flex"
                canvasContainer.style.flexDirection = "column"
                canvasContainer.style.minHeight = "640px"
                canvasContainer.style.maxHeight = "100%"
                this.root.appendChild(canvasContainer)

                this.editor = new EditorCanvas<DiagramNodeData>({
                        initialComponents: [],
                        gridSpacing: 80,
                        getComponentLabel: (component) => component.data?.text ?? `Node ${component.id}`,
                        createComponent: this.createComponentFromPalette.bind(this),
                        renderComponent: this.renderComponent.bind(this),
                        onComponentsChange: () => this.schedulePersist(),
                        onConnectionsChange: () => this.schedulePersist()
                })
                this.editor.root.style.flex = "1 1 auto"
                this.editor.root.style.minHeight = "0"
                canvasContainer.appendChild(this.editor.root)

                this.buildPalette()
                this.registerCanvasInteractions()
                this.restoreState()
        }

        private buildPalette(): void {
                this.palette.root.innerHTML = ""
                this.addPaletteSection("Flowchart Shapes", FLOWCHART_SHAPES)
                this.addPaletteSection("ER Model", ER_MODEL_SHAPES)
        }

        private addPaletteSection(title: string, shapes: FlowchartShape[]): void {
                const section = document.createElement("div")
                section.style.display = "flex"
                section.style.flexDirection = "column"
                section.style.gap = "8px"

                const heading = document.createElement("h3")
                heading.textContent = title
                heading.style.margin = "0"
                heading.style.fontSize = "14px"
                heading.style.textTransform = "uppercase"
                heading.style.color = "#475569"
                section.appendChild(heading)

                for (const shape of shapes) {
                        const config = SHAPE_CONFIG[shape]
                        const item = document.createElement("div")
                        item.textContent = config.label
                        item.draggable = true
                        item.dataset.component = shape
                        item.style.padding = "10px 12px"
                        item.style.borderRadius = "10px"
                        item.style.border = "1px solid #cbd5e1"
                        item.style.background = "#ffffff"
                        item.style.cursor = "grab"
                        item.style.fontSize = "14px"
                        item.style.userSelect = "none"
                        item.addEventListener("dragstart", (event) => {
                                if (!event.dataTransfer) {
                                        return
                                }
                                event.dataTransfer.setData("component", shape)
                                event.dataTransfer.effectAllowed = "copy"
                        })
                        item.addEventListener("click", (event) => {
                                event.preventDefault()
                                const bounds = this.editor.canvasElement.getBoundingClientRect()
                                const position = this.editor.toWorldPoint(
                                        bounds.left + bounds.width / 2,
                                        bounds.top + bounds.height / 2
                                )
                                this.spawnComponent(shape, position)
                        })
                        section.appendChild(item)
                }

                this.palette.root.appendChild(section)
        }

        private registerCanvasInteractions(): void {
                this.editor.canvasElement.addEventListener("dblclick", async (event) => {
                        const { x, y } = this.editor.toWorldPoint(event.clientX, event.clientY)
                        const component = this.findComponentAt(x, y)
                        if (!component) {
                                return
                        }
                        if (component.data?.shape === "entity") {
                                await this.openEntityEditor(component)
                        } else {
                                const result = await showTextPromptModal({
                                        title: "Edit node label",
                                        initialValue: component.data?.text ?? "",
                                        placeholder: "Enter node text"
                                })
                                if (result !== null) {
                                        this.updateComponentData(component.id, {
                                                shape: component.data?.shape ?? "process",
                                                text: result,
                                                erAttributes: component.data?.erAttributes
                                        })
                                }
                        }
                })
        }

        private createComponentFromPalette(
                type: string,
                position: { x: number; y: number },
                helpers: { createId: () => number }
        ): (Omit<DiagramComponent, "id"> & Partial<Pick<DiagramComponent, "id">>) | null {
                if (!this.isDiagramShape(type)) {
                        return null
                }
                return this.buildComponentDefinition(type, position, helpers.createId())
        }

        private spawnComponent(shape: FlowchartShape, position: { x: number; y: number }) {
                const component = this.buildComponentDefinition(shape, position)
                if (component) {
                        this.editor.addComponent(component)
                }
        }

        private buildComponentDefinition(
                shape: FlowchartShape,
                position: { x: number; y: number },
                id?: number
        ): Omit<DiagramComponent, "id"> & Partial<Pick<DiagramComponent, "id">> {
                const config = this.getShapeConfig(shape)
                return {
                        id,
                        x: position.x - config.width / 2,
                        y: position.y - config.height / 2,
                        width: config.width,
                        height: config.height,
                        data: {
                                shape,
                                text: config.defaultText,
                                erAttributes: shape === "entity" ? [] : undefined
                        }
                }
        }

        private renderComponent(
                ctx: CanvasRenderingContext2D,
                component: DiagramComponent,
                state: { selected: boolean }
        ): void {
                const data = component.data ?? {
                        shape: "process" as FlowchartShape,
                        text: ""
                }
                switch (data.shape) {
                        case "startEnd":
                                this.drawRoundedRect(ctx, component, state.selected, data.text, 30)
                                break
                        case "process":
                                this.drawRoundedRect(ctx, component, state.selected, data.text, 6)
                                break
                        case "decision":
                                this.drawDecision(ctx, component, state.selected, data.text)
                                break
                        case "inputOutput":
                                this.drawParallelogram(ctx, component, state.selected, data.text)
                                break
                        case "predefinedProcess":
                                this.drawPredefinedProcess(ctx, component, state.selected, data.text)
                                break
                        case "manualInput":
                                this.drawManualInput(ctx, component, state.selected, data.text)
                                break
                        case "document":
                                this.drawDocument(ctx, component, state.selected, data.text)
                                break
                        case "database":
                                this.drawDatabase(ctx, component, state.selected, data.text)
                                break
                        case "entity":
                                this.drawEntity(ctx, component, state.selected, data)
                                break
                        default:
                                this.drawRoundedRect(ctx, component, state.selected, data.text, 6)
                                break
                }
        }

        private drawRoundedRect(
                ctx: CanvasRenderingContext2D,
                component: DiagramComponent,
                selected: boolean,
                text: string,
                radius: number
        ) {
                ctx.save()
                const stroke = selected ? "#2563eb" : "#0f172a"
                ctx.fillStyle = "#ffffff"
                ctx.strokeStyle = stroke
                ctx.lineWidth = selected ? 3 : 2
                this.roundRectPath(ctx, component, radius)
                ctx.fill()
                ctx.stroke()
                this.drawCenteredText(ctx, text, component, "#0f172a")
                ctx.restore()
        }

        private drawDecision(ctx: CanvasRenderingContext2D, component: DiagramComponent, selected: boolean, text: string) {
                ctx.save()
                const stroke = selected ? "#2563eb" : "#0f172a"
                ctx.fillStyle = "#ffffff"
                ctx.strokeStyle = stroke
                ctx.lineWidth = selected ? 3 : 2
                const centerX = component.x + component.width / 2
                const centerY = component.y + component.height / 2
                ctx.beginPath()
                ctx.moveTo(centerX, component.y)
                ctx.lineTo(component.x + component.width, centerY)
                ctx.lineTo(centerX, component.y + component.height)
                ctx.lineTo(component.x, centerY)
                ctx.closePath()
                ctx.fill()
                ctx.stroke()
                this.drawCenteredText(ctx, text, component, "#0f172a")
                ctx.restore()
        }

        private drawParallelogram(ctx: CanvasRenderingContext2D, component: DiagramComponent, selected: boolean, text: string) {
                ctx.save()
                const offset = component.width * 0.2
                const stroke = selected ? "#2563eb" : "#0f172a"
                ctx.fillStyle = "#ffffff"
                ctx.strokeStyle = stroke
                ctx.lineWidth = selected ? 3 : 2
                ctx.beginPath()
                ctx.moveTo(component.x + offset, component.y)
                ctx.lineTo(component.x + component.width, component.y)
                ctx.lineTo(component.x + component.width - offset, component.y + component.height)
                ctx.lineTo(component.x, component.y + component.height)
                ctx.closePath()
                ctx.fill()
                ctx.stroke()
                this.drawCenteredText(ctx, text, component, "#0f172a")
                ctx.restore()
        }

        private drawPredefinedProcess(
                ctx: CanvasRenderingContext2D,
                component: DiagramComponent,
                selected: boolean,
                text: string
        ) {
                ctx.save()
                const stroke = selected ? "#2563eb" : "#0f172a"
                ctx.fillStyle = "#ffffff"
                ctx.strokeStyle = stroke
                ctx.lineWidth = selected ? 3 : 2
                this.roundRectPath(ctx, component, 6)
                ctx.fill()
                ctx.stroke()
                const inset = component.width * 0.15
                ctx.beginPath()
                ctx.moveTo(component.x + inset, component.y)
                ctx.lineTo(component.x + inset, component.y + component.height)
                ctx.moveTo(component.x + component.width - inset, component.y)
                ctx.lineTo(component.x + component.width - inset, component.y + component.height)
                ctx.lineWidth = 2
                ctx.stroke()
                this.drawCenteredText(ctx, text, component, "#0f172a")
                ctx.restore()
        }

        private drawManualInput(
                ctx: CanvasRenderingContext2D,
                component: DiagramComponent,
                selected: boolean,
                text: string
        ) {
                ctx.save()
                const stroke = selected ? "#2563eb" : "#0f172a"
                ctx.fillStyle = "#ffffff"
                ctx.strokeStyle = stroke
                ctx.lineWidth = selected ? 3 : 2
                const offset = component.height * 0.25
                ctx.beginPath()
                ctx.moveTo(component.x, component.y + offset)
                ctx.lineTo(component.x + component.width, component.y)
                ctx.lineTo(component.x + component.width, component.y + component.height)
                ctx.lineTo(component.x, component.y + component.height)
                ctx.closePath()
                ctx.fill()
                ctx.stroke()
                this.drawCenteredText(ctx, text, component, "#0f172a")
                ctx.restore()
        }

        private drawDocument(ctx: CanvasRenderingContext2D, component: DiagramComponent, selected: boolean, text: string) {
                ctx.save()
                const stroke = selected ? "#2563eb" : "#0f172a"
                ctx.fillStyle = "#ffffff"
                ctx.strokeStyle = stroke
                ctx.lineWidth = selected ? 3 : 2
                const waveHeight = component.height * 0.15
                ctx.beginPath()
                ctx.moveTo(component.x, component.y)
                ctx.lineTo(component.x + component.width, component.y)
                ctx.lineTo(component.x + component.width, component.y + component.height - waveHeight)
                ctx.quadraticCurveTo(
                        component.x + component.width * 0.75,
                        component.y + component.height,
                        component.x + component.width / 2,
                        component.y + component.height - waveHeight / 2
                )
                ctx.quadraticCurveTo(
                        component.x + component.width * 0.25,
                        component.y + component.height - waveHeight,
                        component.x,
                        component.y + component.height
                )
                ctx.closePath()
                ctx.fill()
                ctx.stroke()
                this.drawCenteredText(ctx, text, component, "#0f172a")
                ctx.restore()
        }

        private drawDatabase(ctx: CanvasRenderingContext2D, component: DiagramComponent, selected: boolean, text: string) {
                ctx.save()
                const stroke = selected ? "#2563eb" : "#0f172a"
                ctx.fillStyle = "#ffffff"
                ctx.strokeStyle = stroke
                ctx.lineWidth = selected ? 3 : 2
                const radiusY = component.height * 0.15
                ctx.beginPath()
                ctx.moveTo(component.x, component.y + radiusY)
                ctx.quadraticCurveTo(component.x, component.y, component.x + component.width / 2, component.y)
                ctx.quadraticCurveTo(
                        component.x + component.width,
                        component.y,
                        component.x + component.width,
                        component.y + radiusY
                )
                ctx.lineTo(component.x + component.width, component.y + component.height - radiusY)
                ctx.quadraticCurveTo(
                        component.x + component.width,
                        component.y + component.height,
                        component.x + component.width / 2,
                        component.y + component.height
                )
                ctx.quadraticCurveTo(
                        component.x,
                        component.y + component.height,
                        component.x,
                        component.y + component.height - radiusY
                )
                ctx.closePath()
                ctx.fill()
                ctx.stroke()
                ctx.beginPath()
                ctx.ellipse(
                        component.x + component.width / 2,
                        component.y + radiusY,
                        component.width / 2,
                        radiusY,
                        0,
                        0,
                        Math.PI * 2
                )
                ctx.stroke()
                this.drawCenteredText(ctx, text, component, "#0f172a")
                ctx.restore()
        }

        private drawEntity(ctx: CanvasRenderingContext2D, component: DiagramComponent, selected: boolean, data: DiagramNodeData) {
                ctx.save()
                const stroke = selected ? "#2563eb" : "#b45309"
                const headerHeight = component.height * 0.35
                ctx.fillStyle = "#fef3c7"
                ctx.strokeStyle = stroke
                ctx.lineWidth = selected ? 3 : 2
                this.roundRectPath(ctx, component, 8)
                ctx.fill()
                ctx.stroke()

                ctx.fillStyle = "#f59e0b"
                ctx.strokeStyle = "#d97706"
                ctx.lineWidth = 1.5
                const headerRect: DiagramComponent = {
                        ...component,
                        height: headerHeight
                }
                this.roundRectPath(ctx, headerRect, 8, { topLeft: true, topRight: true, bottomLeft: false, bottomRight: false })
                ctx.fill()
                ctx.stroke()

                this.drawCenteredText(ctx, data.text, headerRect, SHAPE_CONFIG.entity.textColor ?? "#78350f", 18)

                const attributes = data.erAttributes ?? []
                if (attributes.length > 0) {
                        ctx.font = "14px Inter, Arial, sans-serif"
                        ctx.textAlign = "left"
                        ctx.textBaseline = "top"
                        ctx.fillStyle = "#0f172a"
                        const startX = component.x + 16
                        let currentY = component.y + headerHeight + 12
                        for (const attribute of attributes) {
                                const modifiers: string[] = []
                                if (attribute.isPrimaryKey) modifiers.push("PK")
                                if (attribute.isForeignKey) modifiers.push("FK")
                                if (attribute.isUnique) modifiers.push("UQ")
                                if (attribute.allowNull === false) modifiers.push("NOT NULL")
                                const suffix = modifiers.length > 0 ? ` (${modifiers.join(", ")})` : ""
                                ctx.fillText(`${attribute.name}: ${attribute.type}${suffix}`.trim(), startX, currentY)
                                currentY += 18
                        }
                }

                ctx.restore()
        }

        private roundRectPath(
                ctx: CanvasRenderingContext2D,
                component: DiagramComponent,
                radius: number,
                options?: { topLeft?: boolean; topRight?: boolean; bottomRight?: boolean; bottomLeft?: boolean }
        ) {
                const r = Math.min(radius, component.width / 2, component.height / 2)
                const tl = options?.topLeft ?? true
                const tr = options?.topRight ?? true
                const br = options?.bottomRight ?? true
                const bl = options?.bottomLeft ?? true
                ctx.beginPath()
                ctx.moveTo(component.x + (tl ? r : 0), component.y)
                ctx.lineTo(component.x + component.width - (tr ? r : 0), component.y)
                if (tr) ctx.quadraticCurveTo(component.x + component.width, component.y, component.x + component.width, component.y + r)
                else ctx.lineTo(component.x + component.width, component.y)
                ctx.lineTo(component.x + component.width, component.y + component.height - (br ? r : 0))
                if (br)
                        ctx.quadraticCurveTo(
                                component.x + component.width,
                                component.y + component.height,
                                component.x + component.width - r,
                                component.y + component.height
                        )
                else ctx.lineTo(component.x + component.width, component.y + component.height)
                ctx.lineTo(component.x + (bl ? r : 0), component.y + component.height)
                if (bl) ctx.quadraticCurveTo(component.x, component.y + component.height, component.x, component.y + component.height - r)
                else ctx.lineTo(component.x, component.y + component.height)
                ctx.lineTo(component.x, component.y + (tl ? r : 0))
                if (tl) ctx.quadraticCurveTo(component.x, component.y, component.x + (tl ? r : 0), component.y)
                else ctx.lineTo(component.x, component.y)
                ctx.closePath()
        }

        private drawCenteredText(
                ctx: CanvasRenderingContext2D,
                text: string,
                component: DiagramComponent,
                color: string,
                fontSize = 16
        ) {
                ctx.save()
                ctx.fillStyle = color
                ctx.font = `${fontSize}px Inter, Arial, sans-serif`
                ctx.textAlign = "center"
                ctx.textBaseline = "middle"
                const lines = this.wrapText(ctx, text || "", component.width - 20)
                const lineHeight = fontSize + 4
                let currentY = component.y + component.height / 2 - ((lines.length - 1) * lineHeight) / 2
                for (const line of lines) {
                        ctx.fillText(line, component.x + component.width / 2, currentY)
                        currentY += lineHeight
                }
                ctx.restore()
        }

        private wrapText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string[] {
                const words = text.split(/\s+/).filter(Boolean)
                if (words.length === 0) {
                        return [""]
                }
                const lines: string[] = []
                let currentLine = words.shift() ?? ""
                for (const word of words) {
                        const testLine = `${currentLine} ${word}`
                        if (ctx.measureText(testLine).width <= maxWidth) {
                                currentLine = testLine
                        } else {
                                lines.push(currentLine)
                                currentLine = word
                        }
                }
                lines.push(currentLine)
                return lines
        }

        private async openEntityEditor(component: DiagramComponent): Promise<void> {
                const currentName = component.data?.text ?? "Entity"
                const name = await showTextPromptModal({
                        title: "Entity name",
                        initialValue: currentName,
                        placeholder: "Enter entity name"
                })
                if (name === null) {
                        return
                }
                const currentAttributes = component.data?.erAttributes ?? []
                const initialValue = currentAttributes
                        .map((attribute) => {
                                const flags: string[] = []
                                if (attribute.isPrimaryKey) flags.push("#")
                                if (attribute.allowNull === false) flags.push("!")
                                return `${attribute.name}:${attribute.type}${flags.join("")}`
                        })
                        .join("\n")
                const attributesRaw = await showTextPromptModal({
                        title: "Entity attributes",
                        initialValue,
                        placeholder: "name:type",
                        description: entityAttributeDescription
                })
                if (attributesRaw === null) {
                        return
                }
                const parsedAttributes = this.parseAttributes(attributesRaw)
                this.updateComponentData(component.id, {
                        shape: "entity",
                        text: name,
                        erAttributes: parsedAttributes
                })
        }

        private parseAttributes(raw: string): ERAttribute[] {
                const attributes: ERAttribute[] = []
                const lines = raw.split(/\r?\n/)
                for (const line of lines) {
                        const trimmed = line.trim()
                        if (!trimmed) {
                                continue
                        }
                        const flagless = trimmed.replace(/[#!]/g, "")
                        const [name, type] = flagless.split(":")
                        if (!name || !type) {
                                continue
                        }
                        const attribute: ERAttribute = {
                                name: name.trim(),
                                type: type.trim()
                        }
                        if (trimmed.includes("#")) {
                                attribute.isPrimaryKey = true
                        }
                        if (trimmed.includes("!")) {
                                attribute.allowNull = false
                        }
                        attributes.push(attribute)
                }
                return attributes
        }

        private updateComponentData(id: number, data: DiagramNodeData): void {
                this.editor.updateComponent(id, {
                        data
                })
        }

        private findComponentAt(x: number, y: number): DiagramComponent | null {
                const components = this.editor.getComponents()
                for (let index = components.length - 1; index >= 0; index -= 1) {
                        const component = components[index]
                        if (x >= component.x && x <= component.x + component.width && y >= component.y && y <= component.y + component.height) {
                                return component
                        }
                }
                return null
        }

        private isDiagramShape(value: string): value is FlowchartShape {
                return Object.prototype.hasOwnProperty.call(SHAPE_CONFIG, value)
        }

        private getShapeConfig(shape: FlowchartShape): ShapeConfig {
                return SHAPE_CONFIG[shape]
        }

        private schedulePersist(): void {
                if (this.persistHandle !== null) {
                        window.clearTimeout(this.persistHandle)
                }
                this.persistHandle = window.setTimeout(() => this.persistState(), 250)
        }

        private persistState(): void {
                this.persistHandle = null
                try {
                        const state: PersistedDiagramState = {
                                components: this.editor.getComponents(),
                                connections: this.editor.getConnections()
                        }
                        window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state))
                } catch (error) {
                        console.warn("Failed to persist diagram state", error)
                }
        }

        private restoreState(): void {
                try {
                        const raw = window.localStorage.getItem(STORAGE_KEY)
                        if (!raw) {
                                return
                        }
                        const parsed = JSON.parse(raw) as Partial<PersistedDiagramState>
                        if (Array.isArray(parsed.components)) {
                                this.editor.setComponents(
                                        parsed.components.map((component) => ({
                                                ...component,
                                                data: component.data ?? {
                                                        shape: "process",
                                                        text: ""
                                                }
                                        }))
                                )
                        }
                        if (Array.isArray(parsed.connections)) {
                                this.editor.setConnections(parsed.connections)
                        }
                } catch (error) {
                        console.warn("Failed to restore diagram state", error)
                }
        }
}

export function createDiagramEditor(): DiagramEditor {
        return new DiagramEditor()
}
