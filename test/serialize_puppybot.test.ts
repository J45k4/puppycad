import { puppybotSchematic } from "../examples/puppybot"
import { deserialize, serialize } from "../src/serilization"

it("Serializes and deserializes a PuppyBot", () => {
	const map = serialize(puppybotSchematic)
	//console.log("map", map)
	const res = deserialize(map)
	console.log("res", res)
	const schematic = res[0]
	expect(schematic?.equal(puppybotSchematic)).toBe(true)
})
