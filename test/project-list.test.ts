import { beforeEach, describe, expect, it } from "bun:test"
import { Window } from "happy-dom"

import { ProjectList } from "../src/project-list"

describe("ProjectList context menu", () => {
	beforeEach(() => {
		const window = new Window()
		globalThis.window = window as unknown as typeof globalThis.window
		globalThis.document = window.document as unknown as Document
		globalThis.HTMLElement = window.HTMLElement as unknown as typeof HTMLElement
	})

	it("prevents the native menu and renders actions when right-clicking an item", () => {
		let renameInvoked = false
		const projectList = new ProjectList(document, {
			getActions: () => [
				{
					label: "Rename",
					onSelect: () => {
						renameInvoked = true
					}
				}
			]
		})

		projectList.setItems([
			{
				kind: "file",
				id: "file-1",
				name: "File"
			}
		])

		const fileElement = projectList.root.querySelector<HTMLElement>("[data-project-item-id='file-1']")
		expect(fileElement).toBeTruthy()
		if (!fileElement) {
			throw new Error("Expected project item to exist")
		}

		const mockRect: DOMRect = {
			x: 0,
			y: 0,
			width: 200,
			height: 24,
			top: 0,
			left: 0,
			right: 200,
			bottom: 24,
			toJSON() {
				return {}
			}
		}

		projectList.root.getBoundingClientRect = () => mockRect
		fileElement.getBoundingClientRect = () => mockRect

		const event = new window.MouseEvent("contextmenu", {
			bubbles: true,
			cancelable: true,
			clientX: 16,
			clientY: 12
		})

		const dispatched = fileElement.dispatchEvent(event)
		expect(dispatched).toBe(false)
		expect(event.defaultPrevented).toBe(true)

		const menuButton = projectList.root.querySelector("button")
		expect(menuButton?.textContent).toBe("Rename")

		menuButton?.dispatchEvent(new window.MouseEvent("click", { bubbles: true }))
		expect(renameInvoked).toBe(true)
	})

	it("respects non-draggable file metadata", () => {
		const projectList = new ProjectList(document)
		projectList.setItems([
			{
				kind: "file",
				id: "file-1",
				name: "Sketches (0 points)",
				metadata: { draggable: false }
			}
		])
		const fileElement = projectList.root.querySelector<HTMLElement>("[data-project-item-id='file-1']")
		expect(fileElement).toBeTruthy()
		if (!fileElement) {
			throw new Error("Expected project item to exist")
		}
		expect(fileElement.draggable).toBe(false)
	})
})
