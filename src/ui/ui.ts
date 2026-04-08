export class UiComponent<T> {
	public readonly root: T

	constructor(root: T) {
		this.root = root
	}
}

export class Container extends UiComponent<HTMLElement> {
	public add(...components: UiComponent<HTMLElement>[]) {
		this.root.append(...components.map((c) => c.root))
	}
}

export class VList extends UiComponent<HTMLDivElement> {
	constructor(args?: {
		style?: Partial<CSSStyleDeclaration>
	}) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		if (args?.style) Object.assign(this.root.style, args.style)
	}

	public add(...components: (UiComponent<HTMLElement> | HTMLElement)[]): this {
		this.root.append(...components.map((c) => (c instanceof HTMLElement ? c : c.root)))
		return this
	}

	public clear() {
		this.root.innerHTML = ""
	}
}

export class HList extends UiComponent<HTMLDivElement> {
	constructor() {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "row"
	}

	public add(...components: (UiComponent<HTMLElement> | HTMLElement)[]) {
		this.root.append(...components.map((c) => (c instanceof HTMLElement ? c : c.root)))
	}
}

export class Button extends UiComponent<HTMLButtonElement> {
	constructor(args: { text: string }) {
		super(document.createElement("button"))
		this.root.textContent = args.text
	}

	public set onClick(callback: () => void) {
		this.root.onclick = callback
	}
}

export class Label extends UiComponent<HTMLLabelElement> {
	constructor(args: { text: string }) {
		super(document.createElement("label"))
		this.root.textContent = args.text
	}
}

type SelectOption = {
	value: string
	text: string
}

export class Select extends UiComponent<HTMLSelectElement> {
	constructor(args: {
		label?: string
		value?: string
		options: SelectOption[]
	}) {
		super(document.createElement("select"))
		for (const option of args.options) {
			const optionEl = document.createElement("option")
			optionEl.value = option.value
			optionEl.textContent = option.text
			this.root.appendChild(optionEl)
		}
		this.root.value = args.value || ""
	}

	public get value(): string {
		return this.root.value
	}

	public set value(value: string) {
		this.root.value = value
	}

	public set onChange(callback: (value: string) => void) {
		this.root.onchange = () => callback(this.root.value)
	}
}

export class SelectGroup extends UiComponent<HTMLDivElement> {
	private select: Select

	constructor(args: {
		label: string
		value: string
		options: SelectOption[]
	}) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		const labelEl = document.createElement("label")
		labelEl.textContent = args.label
		this.root.appendChild(labelEl)
		this.select = new Select({
			value: args.value,
			options: args.options
		})
		this.root.appendChild(this.select.root)
	}

	public get value(): string {
		return this.select.value
	}

	public set value(value: string) {
		this.select.value = value
	}

	public set onChange(callback: (value: string) => void) {
		this.select.onChange = callback
	}
}

export class TextInput extends UiComponent<HTMLDivElement> {
	private input: HTMLInputElement

	constructor(args: {
		label?: string
		value?: string
		placeholder?: string
	}) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"

		if (args.label) {
			const labelEl = document.createElement("label")
			labelEl.textContent = args.label
			this.root.appendChild(labelEl)
		}

		this.input = document.createElement("input")
		this.input.type = "text"
		this.input.placeholder = args.placeholder || ""
		this.input.value = args.value || ""
		this.root.appendChild(this.input)
	}

	public get value(): string {
		return this.input.value
	}
}

type ModalActionType = "primary" | "secondary" | "danger"

type ModalAction = {
	label: string
	type?: ModalActionType
	onClick: () => void
}

export class Modal extends UiComponent<HTMLDivElement> {
	private dialog: HTMLDivElement
	private body: HTMLDivElement
	private actionsContainer: HTMLDivElement
	private isOpen = false
	private closeCallbacks: (() => void)[] = []
	private previouslyFocused: Element | null = null

	constructor(args?: { title?: string; content?: HTMLElement | UiComponent<HTMLElement> }) {
		super(document.createElement("div"))

		this.root.className = "modal-backdrop"

		this.dialog = document.createElement("div")
		this.dialog.className = "modal-dialog"

		if (args?.title) {
			const titleEl = document.createElement("h2")
			titleEl.textContent = args.title
			titleEl.className = "modal-title"
			this.dialog.appendChild(titleEl)
		}

		this.body = document.createElement("div")
		this.body.className = "modal-body"
		if (args?.content) {
			this.setContent(args.content)
		}
		this.dialog.appendChild(this.body)

		this.actionsContainer = document.createElement("div")
		this.actionsContainer.className = "modal-actions"
		this.dialog.appendChild(this.actionsContainer)

		this.root.appendChild(this.dialog)

		this.root.addEventListener("mousedown", (event) => {
			if (event.target === this.root) {
				event.preventDefault()
				this.close()
			}
		})

		this.handleKeydown = this.handleKeydown.bind(this)
	}

	private handleKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			event.preventDefault()
			this.close()
		}
	}

	public open(parent: HTMLElement = document.body) {
		if (this.isOpen) return
		this.isOpen = true
		this.previouslyFocused = document.activeElement
		parent.appendChild(this.root)
		document.addEventListener("keydown", this.handleKeydown)
	}

	public close() {
		if (!this.isOpen) return
		this.isOpen = false
		this.root.remove()
		document.removeEventListener("keydown", this.handleKeydown)
		for (const callback of this.closeCallbacks) {
			callback()
		}
		const previousElement = this.previouslyFocused
		if (previousElement instanceof HTMLElement) {
			previousElement.focus({ preventScroll: true })
		}
	}

	public show(parent?: HTMLElement) {
		this.open(parent)
	}

	public hide() {
		this.close()
	}

	public onClose(callback: () => void) {
		this.closeCallbacks.push(callback)
	}

	public setContent(content: HTMLElement | UiComponent<HTMLElement>) {
		this.body.innerHTML = ""
		const element = content instanceof HTMLElement ? content : content.root
		this.body.appendChild(element)
	}

	public addAction(action: ModalAction) {
		const button = document.createElement("button")
		button.type = "button"
		button.textContent = action.label
		button.classList.add("button", "modal-button")

		switch (action.type) {
			case "primary":
				button.classList.add("button--primary")
				break
			case "danger":
				button.classList.add("button--danger")
				break
			case "secondary":
				button.classList.add("button--secondary")
				break
			default:
				button.classList.add("button--ghost")
		}

		button.onclick = () => action.onClick()
		this.actionsContainer.appendChild(button)
	}
}

export function showTextPromptModal(args: {
	title: string
	initialValue?: string
	placeholder?: string
	description?: string
	confirmText?: string
	cancelText?: string
}): Promise<string | null> {
	return new Promise((resolve) => {
		const modal = new Modal({ title: args.title })
		let isResolved = false

		const safeResolve = (value: string | null) => {
			if (isResolved) return
			isResolved = true
			resolve(value)
		}

		const container = document.createElement("form")
		container.className = "modal-form"

		if (args.description) {
			const descriptionEl = document.createElement("p")
			descriptionEl.textContent = args.description
			descriptionEl.className = "modal-description"
			container.appendChild(descriptionEl)
		}

		const input = document.createElement("input")
		input.type = "text"
		input.value = args.initialValue ?? ""
		input.placeholder = args.placeholder ?? ""
		input.classList.add("modal-input")

		container.appendChild(input)

		const submit = () => {
			safeResolve(input.value)
			modal.close()
		}

		const cancel = () => {
			safeResolve(null)
			modal.close()
		}

		container.addEventListener("submit", (event) => {
			event.preventDefault()
			submit()
		})

		modal.setContent(container)

		modal.addAction({
			label: args.cancelText ?? "Cancel",
			type: "secondary",
			onClick: cancel
		})

		modal.addAction({
			label: args.confirmText ?? "Save",
			type: "primary",
			onClick: submit
		})

		modal.onClose(() => {
			safeResolve(null)
		})

		modal.open()
		input.focus()
		input.select()
	})
}

export class MultiCheckboxSelect extends UiComponent<HTMLDivElement> {
	private checkboxes: HTMLInputElement[] = []
	private checkboxContainer: HTMLDivElement

	constructor(args: {
		label?: string
		options: { value: string; text: string; checked?: boolean }[]
		expanded?: boolean
	}) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"

		const isExpanded = args.expanded !== undefined ? args.expanded : false

		const header = document.createElement("div")
		header.style.display = "flex"
		header.style.alignItems = "center"
		header.style.cursor = "pointer"

		const toggleIcon = document.createElement("span")
		toggleIcon.textContent = isExpanded ? "▾" : "▸"
		toggleIcon.style.marginRight = "5px"

		if (args.label) {
			const labelEl = document.createElement("label")
			labelEl.textContent = args.label
			header.appendChild(toggleIcon)
			header.appendChild(labelEl)
		} else {
			header.appendChild(toggleIcon)
		}

		this.root.appendChild(header)

		this.checkboxContainer = document.createElement("div")
		this.checkboxContainer.style.display = isExpanded ? "flex" : "none"
		this.checkboxContainer.style.flexDirection = "column"
		this.checkboxContainer.style.maxHeight = "200px"
		this.checkboxContainer.style.overflowY = "auto"
		this.root.appendChild(this.checkboxContainer)

		for (const option of args.options) {
			const container = document.createElement("div")
			container.style.display = "flex"
			container.style.alignItems = "center"

			const checkbox = document.createElement("input")
			checkbox.type = "checkbox"
			checkbox.value = option.value
			checkbox.checked = option.checked || false

			const optionLabel = document.createElement("span")
			optionLabel.textContent = option.text
			optionLabel.style.marginLeft = "5px"

			container.appendChild(checkbox)
			container.appendChild(optionLabel)
			this.checkboxContainer.appendChild(container)

			this.checkboxes.push(checkbox)
		}

		header.onclick = () => {
			const isVisible = this.checkboxContainer.style.display !== "none"
			this.checkboxContainer.style.display = isVisible ? "none" : "flex"
			toggleIcon.textContent = isVisible ? "▸" : "▾"
		}
	}

	public get values(): string[] {
		return this.checkboxes.filter((chk) => chk.checked).map((chk) => chk.value)
	}

	public set onChange(callback: () => void) {
		for (const checkbox of this.checkboxes) {
			checkbox.onchange = callback
		}
	}
}

export class InfiniteScroll extends UiComponent<HTMLElement> {
	private isLoading: boolean
	private onLoadMoreCallback: (() => Promise<void>) | null
	private sentinel: HTMLElement
	private observer: IntersectionObserver

	constructor(args: { container: UiComponent<HTMLElement> }) {
		super(document.createElement("div"))
		this.root.style.minHeight = "100px"

		// Append the custom container to our own root
		this.root.appendChild(args.container.root)
		this.isLoading = false
		this.onLoadMoreCallback = null

		// Create the sentinel element at the bottom
		this.sentinel = document.createElement("div")
		this.sentinel.style.height = "1px"
		this.sentinel.style.marginTop = "1px"
		this.root.appendChild(this.sentinel)

		// Use the custom container’s root as the observer's root
		const observerRoot = args.container.root
		const options: IntersectionObserverInit = {
			threshold: 0.1
		}
		this.observer = new IntersectionObserver((entries: IntersectionObserverEntry[]) => {
			for (const entry of entries) {
				if (!this.isLoading && entry.isIntersecting) {
					this.loadMore()
				}
			}
		}, options)
		this.observer.observe(this.sentinel)
	}

	private async loadMore(): Promise<void> {
		this.isLoading = true
		if (this.onLoadMoreCallback) {
			await this.onLoadMoreCallback()
		}
		this.isLoading = false
	}

	public set onLoadMore(callback: () => Promise<void>) {
		this.onLoadMoreCallback = callback
	}
}

export class Header extends UiComponent<HTMLDivElement> {
	constructor(args: {
		title: string
		rightSide?: UiComponent<HTMLElement>
	}) {
		super(document.createElement("div"))
		this.root.className = "page-header"
		const title = document.createElement("h1")
		title.textContent = args.title
		title.style.flexGrow = "1"
		this.root.appendChild(title)
		if (args.rightSide) {
			this.root.append(args.rightSide.root)
		}
	}
}

export class WrapList implements UiComponent<HTMLDivElement> {
	public readonly root: HTMLDivElement

	constructor() {
		this.root = document.createElement("div")
		this.root.style.display = "flex"
		this.root.style.flexDirection = "row"
		this.root.style.flexWrap = "wrap"
		this.root.style.gap = "5px"
		this.root.style.overflowX = "auto"
		this.root.style.padding = "16px"
	}

	public add(device: UiComponent<HTMLElement>) {
		this.root.appendChild(device.root)
	}

	public set status(message: string) {
		this.root.innerHTML = `<p>${message}</p>`
	}

	public clear() {
		this.root.innerHTML = ""
	}
}

type KeyValue = {
	key: string
	value: string
	href?: string
}

export class KeyValueTable extends VList {
	constructor(items: KeyValue[]) {
		super()
		this.root.className = "list-row"
		for (const item of items) {
			const container = document.createElement("div")
			container.className = "table-cell"
			container.style.fontWeight = "bold"
			this.root.appendChild(container)
			const key = document.createElement("strong")
			key.textContent = item.key
			container.appendChild(key)
			if (item.href) {
				const link = document.createElement("a")
				link.href = item.href
				link.textContent = item.value
				container.appendChild(link)
			} else {
				container.appendChild(document.createTextNode(`: ${item.value}`))
			}
		}
	}
}

export class Collapsible extends UiComponent<HTMLDivElement> {
	private expandButton: HTMLButtonElement
	private content: UiComponent<HTMLElement>
	private contentContainer: HTMLDivElement
	private isOpen: boolean

	constructor(args: {
		buttonText: string
		content: UiComponent<HTMLElement>
	}) {
		super(document.createElement("div"))
		// Position the root relative, so we can absolutely position the content
		this.root.style.position = "relative"

		// Create the expand button
		this.expandButton = document.createElement("button")
		this.expandButton.textContent = args.buttonText
		this.expandButton.style.cursor = "pointer"
		this.root.appendChild(this.expandButton)

		// Store the user-defined content
		this.content = args.content

		// Create a container for the content with absolute positioning
		this.contentContainer = document.createElement("div")
		this.contentContainer.style.position = "absolute"
		// Default to the right side
		this.contentContainer.style.top = "0"
		this.contentContainer.style.left = "100%"
		this.contentContainer.style.zIndex = "1000"
		// Hide by default
		this.contentContainer.style.display = "none"

		// Add the content's root into the container
		this.contentContainer.appendChild(this.content.root)
		this.root.appendChild(this.contentContainer)

		this.isOpen = false

		// Toggle the content on button click
		this.expandButton.addEventListener("click", (e: MouseEvent) => {
			e.stopPropagation()
			this.toggle()
		})

		// Hide the content if clicked outside
		document.addEventListener("click", this.handleDocumentClick.bind(this))
	}

	private toggle(): void {
		if (this.isOpen) {
			this.hide()
		} else {
			this.show()
		}
	}

	private show(): void {
		this.isOpen = true
		this.contentContainer.style.display = "block"

		// Reset to default (open on right)
		this.contentContainer.style.left = "100%"
		this.contentContainer.style.right = "auto"

		// Measure if it goes offscreen
		const rect = this.contentContainer.getBoundingClientRect()
		if (rect.right > window.innerWidth) {
			// Flip to open on the left
			this.contentContainer.style.left = "auto"
			this.contentContainer.style.right = "100%"
		}
	}

	private hide(): void {
		this.isOpen = false
		this.contentContainer.style.display = "none"
	}

	private handleDocumentClick(e: MouseEvent): void {
		if (!this.root.contains(e.target as Node)) {
			this.hide()
		}
	}
}

export class ItemList<T> extends UiComponent<HTMLDivElement> {
	private items: UiComponent<HTMLElement>[] = []

	constructor(args: {
		onClick?: (value: T) => void
		items: {
			label: string
			value: T
			href?: string
		}[]
	}) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.gap = "5px"

		const onClick = args.onClick
		for (const item of args.items) {
			const container = document.createElement("div")
			container.className = "itemlistItem"
			container.style.display = "flex"
			container.style.alignItems = "center"
			container.style.gap = "5px"

			const label = document.createElement("span")
			label.textContent = item.label
			label.style.fontWeight = "bold"

			// if (item.href) {
			// 	const link = document.createElement("a")
			// 	link.href = item.href
			// 	link.textContent = item.value
			// 	value.appendChild(link)
			// }

			if (onClick) {
				container.style.cursor = "pointer"
				container.onclick = () => onClick(item.value)
			}

			container.appendChild(label)
			this.root.appendChild(container)
			this.items.push(new UiComponent(container))
		}
	}
}

export type TreeNode<T> = {
	label: string
	value: T
	children?: TreeNode<T>[]
}

type TreeListRenderCallback<T> = (value: T, elements: { header: HTMLDivElement; container?: HTMLDivElement }) => void

export class TreeList<T> extends UiComponent<HTMLDivElement> {
	private onClick?: (value: T) => void
	private headers: Map<T, HTMLDivElement> = new Map()
	private childContainers: Map<T, HTMLDivElement> = new Map()
	private selectedValue?: T
	private onRenderNode?: TreeListRenderCallback<T>

	public constructor(args: {
		items: TreeNode<T>[]
		onClick?: (value: T) => void
		onRenderNode?: TreeListRenderCallback<T>
	}) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.gap = "5px"
		this.onClick = args.onClick
		this.onRenderNode = args.onRenderNode
		this.setItems(args.items)
	}

	private createNode(item: TreeNode<T>, level: number) {
		if (typeof console !== "undefined") {
			console.log("[TreeList] createNode", {
				label: item.label,
				level,
				hasChildren: Boolean(item.children?.length)
			})
		}
		const container = document.createElement("div")
		container.style.display = "flex"
		container.style.flexDirection = "column"

		const header = document.createElement("div")
		header.style.display = "flex"
		header.style.alignItems = "center"
		header.style.cursor = "pointer"
		header.style.paddingLeft = `${level * 16}px`

		let toggleIcon: HTMLSpanElement | null = null
		if (item.children && item.children.length > 0) {
			toggleIcon = document.createElement("span")
			toggleIcon.textContent = "▾"
			toggleIcon.style.marginRight = "5px"
			header.appendChild(toggleIcon)
		} else {
			const spacer = document.createElement("span")
			spacer.style.display = "inline-block"
			spacer.style.width = "12px"
			spacer.style.marginRight = "5px"
			header.appendChild(spacer)
		}

		const labelEl = document.createElement("span")
		labelEl.textContent = item.label
		header.appendChild(labelEl)
		container.appendChild(header)
		this.headers.set(item.value, header)

		let childrenContainer: HTMLDivElement | undefined
		if (item.children && item.children.length > 0) {
			childrenContainer = document.createElement("div")
			childrenContainer.style.display = "flex"
			childrenContainer.style.flexDirection = "column"
			for (const child of item.children) {
				childrenContainer.appendChild(this.createNode(child, level + 1))
			}
			container.appendChild(childrenContainer)
			this.childContainers.set(item.value, childrenContainer)

			header.onclick = (e: MouseEvent) => {
				e.stopPropagation()
				const targetContainer = childrenContainer
				if (!targetContainer) {
					return
				}
				const isVisible = targetContainer.style.display !== "none"
				targetContainer.style.display = isVisible ? "none" : "flex"
				if (toggleIcon) toggleIcon.textContent = isVisible ? "▸" : "▾"
				if (this.onClick) this.onClick(item.value)
			}
		} else if (this.onClick) {
			const onClick = this.onClick
			header.onclick = () => onClick(item.value)
		}

		this.onRenderNode?.(item.value, { header, container: childrenContainer })

		return container
	}

	public setItems(items: TreeNode<T>[]): void {
		if (typeof console !== "undefined") {
			console.log("[TreeList] setItems:start", { count: items.length })
		}
		this.headers.clear()
		this.childContainers.clear()
		this.selectedValue = undefined
		// Clear existing nodes
		this.root.innerHTML = ""
		// Render new tree nodes
		for (const item of items) {
			this.root.appendChild(this.createNode(item, 0))
		}
		if (typeof console !== "undefined") {
			console.log("[TreeList] setItems:complete", {
				headers: this.headers.size,
				childContainers: this.childContainers.size
			})
		}
	}

	public setSelected(value: T): void {
		// Un-highlight previous selection
		if (this.selectedValue !== undefined) {
			const prevHeader = this.headers.get(this.selectedValue)
			if (prevHeader) prevHeader.style.backgroundColor = ""
		}
		// Highlight new selection
		const header = this.headers.get(value)
		if (header) {
			header.style.backgroundColor = "#e0e0e0"
			this.selectedValue = value
		}
	}

	public getHeader(value: T): HTMLDivElement | undefined {
		return this.headers.get(value)
	}

	public getChildContainer(value: T): HTMLDivElement | undefined {
		return this.childContainers.get(value)
	}
}
