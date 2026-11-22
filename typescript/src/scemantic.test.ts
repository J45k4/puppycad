import { it } from "bun:test"
import { Component, Net, Pin, Schematic } from "./puppycad"

class Resistor extends Component {
	public pin1: Pin
	public pin2: Pin

	public constructor() {
		super("Resistor")
		this.pin1 = new Pin()
		this.pin2 = new Pin()
		this.addPin(this.pin1)
		this.addPin(this.pin2)
	}
}

class Led extends Component {
	public cathode: Pin
	public anode: Pin
	public constructor() {
		super("Led")
		this.cathode = new Pin()
		this.anode = new Pin()
		this.addPin(this.cathode)
		this.addPin(this.anode)
	}
}

class Battery extends Component {
	public positive: Pin
	public negative: Pin

	public constructor() {
		super("Battery")
		this.positive = new Pin()
		this.negative = new Pin()
		this.addPin(this.positive)
		this.addPin(this.negative)
	}
}

it("simple electronic scemantic", () => {
	const resistor = new Resistor()
	const led = new Led()
	const battery = new Battery()

	const net = new Net("GND")
	net.connect(led.cathode)
	net.connect(battery.negative)

	const net2 = new Net("R1")
	net2.connect(battery.positive)
	net2.connect(resistor.pin1)

	const net3 = new Net("VCC")
	net3.connect(resistor.pin2)
	net3.connect(led.anode)

	const schemantic = new Schematic({
		nets: [net, net2, net3]
	})
})
