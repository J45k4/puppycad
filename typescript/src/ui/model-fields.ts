/** Small accessible form controls shared by the solid and assembly editors. */
export function button(label: string, action: () => void): HTMLButtonElement {
	const node = document.createElement("button")
	node.type = "button"
	node.textContent = label
	node.onclick = action
	node.style.cssText = "padding:7px 10px;border:1px solid #b8c5d2;border-radius:6px;background:white;color:#263544;cursor:pointer"
	return node
}
export function section(parent: HTMLElement, title: string, open = false): HTMLDetailsElement {
	const node = document.createElement("details")
	node.open = open
	node.style.cssText = "border:1px solid #cbd5e1;border-radius:7px;padding:10px;margin:8px 0"
	const summary = document.createElement("summary")
	summary.textContent = title
	summary.style.cssText = "font-weight:600;cursor:pointer;margin-bottom:8px"
	node.append(summary)
	parent.append(node)
	return node
}
export function field(parent: HTMLElement, label: string, value: string | number, change: (value: string) => void, type = "text"): HTMLInputElement {
	const row = document.createElement("label")
	row.style.cssText = "display:flex;gap:8px;align-items:center;justify-content:space-between;margin:6px 0;font-size:13px"
	const title = document.createElement("span")
	title.textContent = label
	const input = document.createElement("input")
	input.type = type
	input.value = String(value)
	input.setAttribute("aria-label", label)
	input.style.cssText = "width:130px;min-width:0;padding:6px;border:1px solid #bac8d5;border-radius:4px;background:white;color:#172b3a"
	input.onchange = () => change(input.value)
	row.append(title, input)
	parent.append(row)
	return input
}
export function numberField(parent: HTMLElement, label: string, value: number, change: (value: number) => void): HTMLInputElement {
	const input = field(
		parent,
		label,
		value,
		(v) => {
			const n = Number(v)
			if (!v.trim() || !Number.isFinite(n)) {
				input.setCustomValidity("Enter a finite number.")
				input.reportValidity()
				return
			}
			input.setCustomValidity("")
			change(n)
		},
		"number"
	)
	input.step = "any"
	input.required = true
	return input
}
export function select(parent: HTMLElement, label: string, value: string, choices: readonly (string | { value: string; label: string })[], change: (value: string) => void): HTMLSelectElement {
	const row = document.createElement("label")
	row.style.cssText = "display:flex;gap:8px;align-items:center;justify-content:space-between;margin:8px 0;font-size:13px"
	row.append(document.createTextNode(label))
	const input = document.createElement("select")
	input.setAttribute("aria-label", label)
	input.style.cssText = "max-width:220px;padding:6px;background:white;color:#172b3a;border:1px solid #bac8d5;border-radius:4px"
	for (const choice of choices) {
		const option = document.createElement("option")
		option.value = typeof choice === "string" ? choice : choice.value
		option.textContent = typeof choice === "string" ? choice : choice.label
		input.append(option)
	}
	input.value = value
	input.onchange = () => change(input.value)
	row.append(input)
	parent.append(row)
	return input
}
export function vector(parent: HTMLElement, label: string, value: { x: number; y: number; z: number }, change: () => void): void {
	const group = section(parent, label)
	for (const axis of ["x", "y", "z"] as const)
		numberField(group, `${label} ${axis.toUpperCase()}`, value[axis], (n) => {
			value[axis] = n
			change()
		})
}
export function note(parent: HTMLElement, text: string): void {
	const p = document.createElement("p")
	p.textContent = text
	p.style.cssText = "font-size:12px;line-height:1.5;color:#526478"
	parent.append(p)
}
export const panelStyle = "width:360px;max-width:48%;flex-shrink:0;overflow:auto;padding:14px;box-sizing:border-box;background:#f8fafc;color:#243648;font:14px system-ui;border-right:1px solid #cbd5e1"

export function validateFields(root: HTMLElement): void {
	for (const input of Array.from(root.querySelectorAll<HTMLInputElement>("input"))) {
		if (input.validity.customError || (input.type === "number" && (!input.value.trim() || !Number.isFinite(Number(input.value)))))
			throw Error(`${input.getAttribute("aria-label")}: enter a valid value.`)
	}
}
