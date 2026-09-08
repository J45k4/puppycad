import { expect, it } from "bun:test"
import { Window } from "happy-dom"
import { ProjectList } from "../src/ui/project-list"
it("restores tree selection without replaying navigation callbacks", () => {
	const dom = new Window()
	const selected: string[] = []
	const list = new ProjectList(dom.document as unknown as Document, { onSelect: ({ id }) => selected.push(id) })
	const items = [{ kind: "folder" as const, id: "part", name: "Part", items: [{ kind: "file" as const, id: "feature", name: "Feature", metadata: { synthetic: true, draggable: false } }] }]
	list.setItems(items, "feature")
	expect(selected).toEqual([])
	list.selectById("part")
	expect(selected).toEqual(["part"])
	list.setItems(items, "part")
	expect(selected).toEqual(["part"])
})
