import { beforeEach, describe, expect, it } from "bun:test"
import { Window } from "happy-dom"

import { ensureProject, listProjects } from "../src/ui/projects"

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
})
