import { beforeEach, describe, expect, it } from "bun:test"
import { Window } from "happy-dom"

import { ensureProject, getProjectNameFromFileName, isViewableImageFile, listProjects, readProjectFile } from "../src/ui/projects"

describe("project metadata", () => {
	beforeEach(() => {
		const window = new Window()
		globalThis.window = window as unknown as typeof globalThis.window
		globalThis.localStorage = window.localStorage as unknown as Storage
	})

	it("creates a local metadata stub for pasted project urls", () => {
		const project = ensureProject("server-project-id")

		expect(project.id).toBe("server-project-id")
		expect(project.name).toBe("Project 1")
		expect(listProjects().map((entry) => entry.id)).toEqual(["server-project-id"])
	})

	it("reuses existing metadata when ensuring a project", () => {
		const first = ensureProject("server-project-id", "Shared Project")
		const second = ensureProject("server-project-id", "Ignored")

		expect(second).toEqual(first)
		expect(listProjects()).toHaveLength(1)
		expect(listProjects()[0]?.name).toBe("Shared Project")
	})

	it("derives project names from opened project files", () => {
		expect(getProjectNameFromFileName("bracket.pcad")).toBe("bracket")
		expect(getProjectNameFromFileName("archive/project.json")).toBe("project")
		expect(getProjectNameFromFileName("   ")).toBe("Untitled Project")
	})

	it("reads and normalizes opened project files", async () => {
		const project = await readProjectFile({
			name: "bracket.pcad",
			text: async () =>
				JSON.stringify({
					version: 4,
					items: [{ id: "part-1", type: "part", name: "Bracket" }],
					selectedPath: [0]
				})
		})

		expect(project.version).toBe(4)
		expect(project.items[0]).toMatchObject({ id: "part-1", type: "part", name: "Bracket" })
		expect(project.selectedPath).toEqual([0])
	})

	it("rejects invalid opened project files", async () => {
		await expect(
			readProjectFile({
				name: "broken.pcad",
				text: async () => "{"
			})
		).rejects.toThrow("valid JSON")

		await expect(
			readProjectFile({
				name: "broken.pcad",
				text: async () => JSON.stringify({ version: 99, items: [] })
			})
		).rejects.toThrow("Invalid PuppyCAD project file")
	})

	it("detects image files as previewable instead of project imports", () => {
		expect(isViewableImageFile({ name: "preview.png", type: "" })).toBe(true)
		expect(isViewableImageFile({ name: "render", type: "image/webp" })).toBe(true)
		expect(isViewableImageFile({ name: "project.pcad", type: "application/json" })).toBe(false)
	})
})
