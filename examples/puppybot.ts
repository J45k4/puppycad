import { Schematic, Net, Component, Pin, PCB } from "../src/puppycad"

// Component definitions
export class BatteryPack extends Component {
    public readonly plus12V: Pin
    constructor() {
        super("BatteryPack")
        this.plus12V = new Pin("plus12V", this)
        this.addPin(this.plus12V)
    }
}

export class PPTCFuse extends Component {
    public readonly IN: Pin
    public readonly OUT: Pin
    constructor() {
        super("PPTCFuse")
        this.IN = new Pin("IN", this)
        this.OUT = new Pin("OUT", this)
        this.addPin(this.IN)
        this.addPin(this.OUT)
    }
}

export class TVSDiode extends Component {
    public readonly VIN: Pin
    public readonly VOUT: Pin
    constructor() {
        super("TVSDiode")
        this.VIN = new Pin("VIN", this)
        this.VOUT = new Pin("VOUT", this)
        this.addPin(this.VIN)
        this.addPin(this.VOUT)
    }
}

export class BQ25703A extends Component {
    public readonly BAT: Pin
    public readonly IN: Pin
    public readonly SYS: Pin
    constructor() {
        super("BQ25703A")
        this.BAT = new Pin("BAT", this)
        this.IN = new Pin("IN", this)
        this.SYS = new Pin("SYS", this)
        this.addPin(this.BAT)
        this.addPin(this.IN)
        this.addPin(this.SYS)
    }
}

export class TPS563201 extends Component {
    public readonly VIN: Pin
    public readonly OUT: Pin
    constructor() {
        super("TPS563201")
        this.VIN = new Pin("VIN", this)
        this.OUT = new Pin("OUT", this)
        this.addPin(this.VIN)
        this.addPin(this.OUT)
    }
}

export class InductorL1 extends Component {
    public readonly L1_IN: Pin
    public readonly L1_OUT: Pin
    constructor() {
        super("InductorL1")
        this.L1_IN = new Pin("L1_IN", this)
        this.L1_OUT = new Pin("L1_OUT", this)
        this.addPin(this.L1_IN)
        this.addPin(this.L1_OUT)
    }
}

export class ESP32_WROOM_32E extends Component {
    public readonly V5_IN: Pin
    public readonly V33: Pin
    public readonly GPIO21: Pin
    public readonly GPIO22: Pin
    constructor() {
        super("ESP32-WROOM-32E")
        this.V5_IN = new Pin("5V_IN", this)
        this.V33 = new Pin("3V3", this)
        this.GPIO21 = new Pin("GPIO21", this)
        this.GPIO22 = new Pin("GPIO22", this)
        this.addPin(this.V5_IN)
        this.addPin(this.V33)
        this.addPin(this.GPIO21)
        this.addPin(this.GPIO22)
    }
}

export class AMS1117_3_3 extends Component {
    public readonly VIN: Pin
    public readonly VOUT: Pin
    constructor() {
        super("AMS1117-3.3")
        this.VIN = new Pin("VIN", this)
        this.VOUT = new Pin("VOUT", this)
        this.addPin(this.VIN)
        this.addPin(this.VOUT)
    }
}

export class OV2640 extends Component {
    public readonly VCC: Pin
    public readonly SDA: Pin
    public readonly SCL: Pin
    constructor() {
        super("OV2640")
        this.VCC = new Pin("VCC", this)
        this.SDA = new Pin("SDA", this)
        this.SCL = new Pin("SCL", this)
        this.addPin(this.VCC)
        this.addPin(this.SDA)
        this.addPin(this.SCL)
    }
}

export class STUSB4500 extends Component {
    public readonly SDA: Pin
    public readonly SCL: Pin
    constructor() {
        super("STUSB4500")
        this.SDA = new Pin("SDA", this)
        this.SCL = new Pin("SCL", this)
        this.addPin(this.SDA)
        this.addPin(this.SCL)
    }
}

// Circuit instantiation
// Define nets
const netBattery = new Net("BATTERY+")
const netBattInput = new Net("BAT_INPUT")
const netSystem12V = new Net("12V")
const net5V = new Net("5V")
const net3V3 = new Net("3V3")
const netGND = new Net("GND")
const netI2C_SDA = new Net("I2C_SDA")
const netI2C_SCL = new Net("I2C_SCL")

// Instantiate components and connect pins
const batteryPack = new BatteryPack()
netBattery.connect(batteryPack.plus12V)

const pptcFuse = new PPTCFuse()
netBattery.connect(pptcFuse.IN)
netBattInput.connect(pptcFuse.OUT)

const tvsDiode = new TVSDiode()
netBattInput.connect(tvsDiode.VIN)
netBattInput.connect(tvsDiode.VOUT)

const charger = new BQ25703A()
netBattery.connect(charger.BAT)
netBattInput.connect(charger.IN)
netSystem12V.connect(charger.SYS)

const buck = new TPS563201()
netSystem12V.connect(buck.VIN)
net5V.connect(buck.OUT)

const inductor = new InductorL1()
netSystem12V.connect(inductor.L1_IN)
net5V.connect(inductor.L1_OUT)

const esp = new ESP32_WROOM_32E()
net5V.connect(esp.V5_IN)
net3V3.connect(esp.V33)
netI2C_SDA.connect(esp.GPIO21)
netI2C_SCL.connect(esp.GPIO22)

const ldo = new AMS1117_3_3()
net5V.connect(ldo.VIN)
net3V3.connect(ldo.VOUT)

const camera = new OV2640()
net3V3.connect(camera.VCC)
netI2C_SDA.connect(camera.SDA)
netI2C_SCL.connect(camera.SCL)

const pdCtrl = new STUSB4500()
netI2C_SDA.connect(pdCtrl.SDA)
netI2C_SCL.connect(pdCtrl.SCL)

// Export schematic
export const puppybotSchematic = new Schematic({
	name: "PuppyBot v0.2",
    nets: [
        netBattery,
        netBattInput,
        netSystem12V,
        net5V,
        net3V3,
        netGND,
        netI2C_SDA,
        netI2C_SCL
    ]
})


export const puppybotPCB = new PCB({
    name: "PuppyBot v0.2 PCB",
    material: "FR4",
    thickness: 1.6
})
// Add board layers
puppybotPCB.addLayer({ name: "TopCopper", type: "copper", material: "copper", thickness: 0.035 })
puppybotPCB.addLayer({ name: "BottomCopper", type: "copper", material: "copper", thickness: 0.035 })
puppybotPCB.addLayer({ name: "SilkscreenTop", type: "silkscreen", material: "silkscreen_ink", thickness: 0.1 })
puppybotPCB.addLayer({ name: "SoldermaskTop", type: "soldermask", material: "epoxy_soldermask", thickness: 0.1 })

// Add components and nets to PCB
puppybotPCB.components.push(
    batteryPack,
    pptcFuse,
    tvsDiode,
    charger,
    buck,
    inductor,
    esp,
    ldo,
    camera,
    pdCtrl
)
puppybotPCB.nets.push(
    netBattery,
    netBattInput,
    netSystem12V,
    net5V,
    net3V3,
    netGND,
    netI2C_SDA,
    netI2C_SCL
)