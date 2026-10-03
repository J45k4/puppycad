import { expect, it } from "bun:test"
import { spawnSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const ROOT = join(import.meta.dir, "..")
const hasCargo = spawnSync("cargo", ["--version"], { encoding: "utf8" }).status === 0

function exportFixture(output: string) {
	const input = join(ROOT, "rust", "fixtures", "extruded-rectangle.pcad")
	return spawnSync("cargo", ["run", "--quiet", "--manifest-path", join(ROOT, "rust", "Cargo.toml"), "--", "--export", input, output], {
		encoding: "utf8",
		timeout: 600_000
	})
}

it.skipIf(!hasCargo)(
	"exports binary STL and 3MF headlessly from the native app",
	() => {
		const dir = mkdtempSync(join(tmpdir(), "puppycad-rust-export-"))
		try {
			const stlPath = join(dir, "block.stl")
			const stlResult = exportFixture(stlPath)
			expect({ status: stlResult.status, stderr: stlResult.stderr }).toEqual({ status: 0, stderr: "" })
			const stl = readFileSync(stlPath)
			expect(stl.subarray(0, 9).toString("latin1")).toBe("PuppyCAD ")
			const count = stl.readUInt32LE(80)
			expect(count).toBe(12)
			expect(stl.length).toBe(84 + count * 50)

			const threeMfPath = join(dir, "block.3mf")
			const threeMfResult = exportFixture(threeMfPath)
			expect({ status: threeMfResult.status, stderr: threeMfResult.stderr }).toEqual({ status: 0, stderr: "" })
			const threeMf = readFileSync(threeMfPath)
			expect([...threeMf.subarray(0, 4)]).toEqual([0x50, 0x4b, 0x03, 0x04])
			const names = threeMf.toString("latin1")
			for (const part of ["[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"]) expect(names).toContain(part)
		} finally {
			rmSync(dir, { recursive: true, force: true })
		}
	},
	600_000
)
