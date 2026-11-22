export type ProjectTreeNodeKind = "file" | "folder"

let nextNodeId = 0

export abstract class ProjectTreeNode {
	public readonly id: string
	public parent: ProjectTreeFolder | null = null
	public constructor(public name: string) {
		this.id = `project-node-${nextNodeId++}`
	}

	public abstract readonly kind: ProjectTreeNodeKind
}

export class ProjectTreeFile extends ProjectTreeNode {
	public override readonly kind: ProjectTreeNodeKind = "file"
}

export class ProjectTreeFolder extends ProjectTreeNode {
	public override readonly kind: ProjectTreeNodeKind = "folder"
	public readonly children: ProjectTreeNode[] = []

	public addChild(node: ProjectTreeNode) {
		this.children.push(node)
		node.parent = this
	}

	public removeChild(node: ProjectTreeNode) {
		const index = this.children.indexOf(node)
		if (index >= 0) {
			this.children.splice(index, 1)
			node.parent = null
		}
	}

	public isAncestorOf(node: ProjectTreeNode): boolean {
		let current = node.parent
		while (current) {
			if (current === this) {
				return true
			}
			current = current.parent
		}
		return false
	}
}

export class ProjectTree {
	public readonly rootItems: ProjectTreeNode[] = []
	private readonly nodeById = new Map<string, ProjectTreeNode>()

	public createFolder(name: string, parent?: ProjectTreeFolder | null): ProjectTreeFolder {
		const folder = new ProjectTreeFolder(name)
		this.register(folder)
		if (parent) {
			parent.addChild(folder)
		} else {
			this.rootItems.push(folder)
		}
		return folder
	}

	public createFile(name: string, parent: ProjectTreeFolder): ProjectTreeFile {
		const file = new ProjectTreeFile(name)
		this.register(file)
		parent.addChild(file)
		return file
	}

	public getNode(id: string): ProjectTreeNode | undefined {
		return this.nodeById.get(id)
	}

	public canMove(node: ProjectTreeNode, destination: ProjectTreeFolder | null): boolean {
		if (!node) {
			return false
		}
		if (!destination) {
			return true
		}
		if (node === destination) {
			return false
		}
		if (node instanceof ProjectTreeFolder && node.isAncestorOf(destination)) {
			return false
		}
		return true
	}

	public move(node: ProjectTreeNode, destination: ProjectTreeFolder | null): boolean {
		if (!this.canMove(node, destination)) {
			return false
		}
		if (node.parent) {
			node.parent.removeChild(node)
		} else {
			this.removeFromRoot(node)
		}

		if (destination) {
			destination.addChild(node)
		} else {
			node.parent = null
			this.rootItems.push(node)
		}

		return true
	}

	private register(node: ProjectTreeNode) {
		this.nodeById.set(node.id, node)
	}

	private removeFromRoot(node: ProjectTreeNode) {
		const index = this.rootItems.indexOf(node)
		if (index >= 0) {
			this.rootItems.splice(index, 1)
		}
	}
}
