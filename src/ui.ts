export class UiComponent<T> {
	public readonly root: T

	constructor(root: T) {
		this.root = root
	}
}

export class Container extends UiComponent<HTMLElement> {
	constructor(root: HTMLElement) {
		super(root)
	}

	public add(...components: UiComponent<HTMLElement>[]) {
		this.root.append(...components.map(c => c.root))
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
		this.root.append(
			...components.map(c => c instanceof HTMLElement ? c : c.root)
		);
		return this;
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
		this.root.append(...components.map(c => c instanceof HTMLElement ? c : c.root))
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
		args.options.forEach(option => {
			const optionEl = document.createElement("option")
			optionEl.value = option.value
			optionEl.textContent = option.text
			this.root.appendChild(optionEl)
		})
		this.root.value = args.value || ""
	}

	public get value(): string {
		return this.root.value
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

export class MultiCheckboxSelect extends UiComponent<HTMLDivElement> {
	private checkboxes: HTMLInputElement[] = []
	private checkboxContainer: HTMLDivElement

	constructor(args: {
		label?: string
		options: { value: string, text: string, checked?: boolean }[]
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
		return this.checkboxes.filter(chk => chk.checked).map(chk => chk.value)
	}

	public set onChange(callback: () => void) {
		this.checkboxes.forEach(checkbox => {
			checkbox.onchange = callback
		})
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
            entries.forEach((entry: IntersectionObserverEntry) => {
                if (!this.isLoading && entry.isIntersecting) {
                    this.loadMore()
                }
            })
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

export class Modal extends UiComponent<HTMLDivElement> {
    private backdrop: HTMLDivElement
    private modalContainer: HTMLDivElement
    private contentContainer: HTMLDivElement
    private closeButton: HTMLButtonElement

    constructor(args: { title?: string; content: UiComponent<HTMLElement> | HTMLElement }) {
        // Create backdrop
        const backdrop = document.createElement("div")
        backdrop.className = "modal-backdrop"
		backdrop.style.display = "none";
		backdrop.style.position = "fixed";
		backdrop.style.top = "0";
		backdrop.style.left = "0";
		backdrop.style.width = "100%";
		backdrop.style.height = "100%";
		backdrop.style.backgroundColor = "rgba(0, 0, 0, 0.5)";
		backdrop.style.alignItems = "center";
		backdrop.style.justifyContent = "center";
		backdrop.style.zIndex = "1000";
		backdrop.onclick = () => this.hide()
        super(backdrop)
        this.backdrop = backdrop

        // Create modal container
        this.modalContainer = document.createElement("div")
		this.modalContainer.onclick = (e: MouseEvent) => e.stopPropagation()

		this.modalContainer.style.backgroundColor = "white"
		this.modalContainer.style.padding = "16px"
		this.modalContainer.style.borderRadius = "4px"
		this.modalContainer.style.maxWidth = "90%"
		this.modalContainer.style.maxHeight = "90%"
		this.modalContainer.style.overflow = "auto"
		this.modalContainer.style.position = "relative"
		this.modalContainer.style.width = "600px"
        backdrop.appendChild(this.modalContainer)

        // Add close button
        this.closeButton = document.createElement("button")
        this.closeButton.textContent = "✖"
        Object.assign(this.closeButton.style, {
            position: "absolute",
            top: "8px",
            right: "8px",
            cursor: "pointer",
            background: "none",
            border: "none",
            fontSize: "16px"
        })
        this.modalContainer.appendChild(this.closeButton)
        this.closeButton.onclick = () => this.hide()

        // Add title if provided
        if (args.title) {
            const titleEl = document.createElement("h2")
            titleEl.textContent = args.title
            titleEl.style.marginTop = "0"
            this.modalContainer.appendChild(titleEl)
        }

        // Add content
        const contentRoot = args.content instanceof HTMLElement ? args.content : args.content.root
        this.contentContainer = document.createElement("div")
        this.contentContainer.appendChild(contentRoot)
        this.modalContainer.appendChild(this.contentContainer)
        document.body.appendChild(this.backdrop)
    }

    public show(): void {
        this.backdrop.style.display = "flex"
    }

    public hide(): void {
        this.backdrop.style.display = "none"
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

			if (args.onClick) {
				container.style.cursor = "pointer"
				container.onclick = () => args.onClick!(item.value)
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

export class TreeList<T> extends UiComponent<HTMLDivElement> {
	private onClick?: (value: T) => void
	private headers: Map<T, HTMLDivElement> = new Map()
	private selectedValue?: T

	public constructor(args: {
		items: TreeNode<T>[]
		onClick?: (value: T) => void
	}) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.gap = "5px"
		this.onClick = args.onClick
		this.setItems(args.items)
	}

	private createNode(item: TreeNode<T>, level: number): HTMLDivDivElement {
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

		let childrenContainer: HTMLDivElement | null = null
		if (item.children && item.children.length > 0) {
			childrenContainer = document.createElement("div")
			childrenContainer.style.display = "flex"
			childrenContainer.style.flexDirection = "column"
			item.children.forEach(child => {
				childrenContainer!.appendChild(this.createNode(child, level + 1))
			})
			container.appendChild(childrenContainer)

			header.onclick = (e: MouseEvent) => {
				e.stopPropagation()
				const isVisible = childrenContainer!.style.display !== "none"
				childrenContainer!.style.display = isVisible ? "none" : "flex"
				if (toggleIcon) toggleIcon.textContent = isVisible ? "▸" : "▾"
				if (this.onClick) this.onClick(item.value)
			}
		} else if (this.onClick) {
			header.onclick = () => this.onClick!(item.value)
		}

		return container
	}

	public setItems(items: TreeNode<T>[]): void {
		this.headers.clear()
		this.selectedValue = undefined
		// Clear existing nodes
		this.root.innerHTML = "";
		// Render new tree nodes
		items.forEach(item => {
			this.root.appendChild(this.createNode(item, 0));
		});
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
}