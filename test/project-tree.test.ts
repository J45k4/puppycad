import { describe, expect, it } from "bun:test"

import { ProjectTree } from "../src/project-tree"

describe("ProjectTree move behaviour", () => {
	it("moves a file into another folder", () => {
		const tree = new ProjectTree()
		const sourceFolder = tree.createFolder("Source")
		const destinationFolder = tree.createFolder("Destination")
		const file = tree.createFile("file", sourceFolder)

		const moved = tree.move(file, destinationFolder)

		expect(moved).toBe(true)
		expect(file.parent).toBe(destinationFolder)
		expect(sourceFolder.children).not.toContain(file)
		expect(destinationFolder.children).toContain(file)
	})

	it("moves a folder into another folder", () => {
		const tree = new ProjectTree()
		const outer = tree.createFolder("Outer")
		const inner = tree.createFolder("Inner")
		const target = tree.createFolder("Target")
		tree.move(inner, outer)

		const moved = tree.move(inner, target)

		expect(moved).toBe(true)
		expect(inner.parent).toBe(target)
		expect(outer.children).not.toContain(inner)
		expect(target.children).toContain(inner)
	})

	it("prevents moving a folder into its descendant", () => {
		const tree = new ProjectTree()
		const parent = tree.createFolder("Parent")
		const child = tree.createFolder("Child", parent)
		const grandchild = tree.createFolder("Grandchild", child)

		const moved = tree.move(parent, grandchild)

		expect(moved).toBe(false)
		expect(parent.parent).toBeNull()
		expect(tree.rootItems).toContain(parent)
		expect(grandchild.parent).toBe(child)
	})

	it("moves a folder to the root list", () => {
		const tree = new ProjectTree()
		const folder = tree.createFolder("Folder")
		const nested = tree.createFolder("Nested", folder)

		const moved = tree.move(nested, null)

		expect(moved).toBe(true)
		expect(nested.parent).toBeNull()
		expect(folder.children).not.toContain(nested)
		expect(tree.rootItems).toContain(nested)
	})

	it("moves a file to the root list", () => {
		const tree = new ProjectTree()
		const folder = tree.createFolder("Folder")
		const file = tree.createFile("File", folder)

		const moved = tree.move(file, null)

		expect(moved).toBe(true)
		expect(file.parent).toBeNull()
		expect(folder.children).not.toContain(file)
		expect(tree.rootItems).toContain(file)
	})

	it("does not duplicate entries when moving repeatedly", () => {
		const tree = new ProjectTree()
		const folderA = tree.createFolder("A")
		const folderB = tree.createFolder("B")
		const file = tree.createFile("File", folderA)

		expect(tree.move(file, folderB)).toBe(true)
		expect(tree.move(file, folderA)).toBe(true)

		expect(folderA.children.filter((child) => child === file).length).toBe(1)
		expect(folderB.children).not.toContain(file)
	})
})
