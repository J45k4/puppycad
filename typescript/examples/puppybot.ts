import { Schematic, Net, Pin, PCB, Entity, Footprint, PadShape, Vec2 } from "../src/puppycad"

export class BatteryPack extends Entity {
	public readonly plus12V = new Pin("plus12V")
	public readonly GND = new Pin("GND")
	public readonly footprint: Footprint
	constructor() {
		super("BatteryPack")
		this.footprint = new Footprint({
			name: "BatteryPack_Conn2",
			pads: [
				{
					pin: this.plus12V,
					x: -1.5,
					y: 0.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.GND,
					x: 1.5,
					y: 0.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-2.0, -1.0), new Vec2(2.0, -1.0), new Vec2(2.0, 1.0), new Vec2(-2.0, 1.0)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class PPTCFuse extends Entity {
	public readonly IN = new Pin("IN")
	public readonly OUT = new Pin("OUT")
	public readonly footprint: Footprint
	constructor() {
		super("PPTCFuse")
		this.footprint = new Footprint({
			name: "PPTCFuse_SMD_2",
			pads: [
				{
					pin: this.IN,
					x: -1.0,
					y: 0.0,
					width: 1.0,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.OUT,
					x: 1.0,
					y: 0.0,
					width: 1.0,
					height: 1.5,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-1.5, -1.0), new Vec2(1.5, -1.0), new Vec2(1.5, 1.0), new Vec2(-1.5, 1.0)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class TVSDiode extends Entity {
	public readonly VIN = new Pin("VIN")
	public readonly VOUT = new Pin("VOUT")
	public readonly footprint: Footprint
	constructor() {
		super("TVSDiode")
		this.footprint = new Footprint({
			name: "TVSDiode_SMD_2",
			pads: [
				{
					pin: this.VIN,
					x: -1.5,
					y: 0.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.VOUT,
					x: 1.5,
					y: 0.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-2.0, -1.0), new Vec2(2.0, -1.0), new Vec2(2.0, 1.0), new Vec2(-2.0, 1.0)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class BQ25703A extends Entity {
	public readonly BAT = new Pin("BAT")
	public readonly IN = new Pin("IN")
	public readonly SYS = new Pin("SYS")
	public readonly P1 = new Pin("P1")
	public readonly P2 = new Pin("P2")
	public readonly P3 = new Pin("P3")
	public readonly P4 = new Pin("P4")
	public readonly P5 = new Pin("P5")
	public readonly P6 = new Pin("P6")
	public readonly P7 = new Pin("P7")
	public readonly P8 = new Pin("P8")
	public readonly P9 = new Pin("P9")
	public readonly P10 = new Pin("P10")
	public readonly P11 = new Pin("P11")
	public readonly P12 = new Pin("P12")
	public readonly P13 = new Pin("P13")
	public readonly P14 = new Pin("P14")
	public readonly P15 = new Pin("P15")
	public readonly P16 = new Pin("P16")
	public readonly P17 = new Pin("P17")
	public readonly P18 = new Pin("P18")
	public readonly P19 = new Pin("P19")
	public readonly P20 = new Pin("P20")
	public readonly P21 = new Pin("P21")
	public readonly P22 = new Pin("P22")
	public readonly P23 = new Pin("P23")
	public readonly P24 = new Pin("P24")
	public readonly P25 = new Pin("P25")
	public readonly P26 = new Pin("P26")
	public readonly P27 = new Pin("P27")
	public readonly P28 = new Pin("P28")
	public readonly P29 = new Pin("P29")
	public readonly P30 = new Pin("P30")
	public readonly P31 = new Pin("P31")
	public readonly P32 = new Pin("P32")
	public readonly footprint: Footprint
	constructor() {
		super("BQ25703A")
		this.footprint = new Footprint({
			name: "BQ25703A_WQFN32",
			pads: [
				// Top side pins 1-8 (left to right)
				{
					pin: this.P1,
					x: -1.75,
					y: 2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P2,
					x: -1.25,
					y: 2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P3,
					x: -0.75,
					y: 2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P4,
					x: -0.25,
					y: 2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P5,
					x: 0.25,
					y: 2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P6,
					x: 0.75,
					y: 2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P7,
					x: 1.25,
					y: 2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P8,
					x: 1.75,
					y: 2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				// Right side pins 9-16 (top to bottom)
				{
					pin: this.P9,
					x: 2.15,
					y: 1.75,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P10,
					x: 2.15,
					y: 1.25,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P11,
					x: 2.15,
					y: 0.75,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P12,
					x: 2.15,
					y: 0.25,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P13,
					x: 2.15,
					y: -0.25,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P14,
					x: 2.15,
					y: -0.75,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P15,
					x: 2.15,
					y: -1.25,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P16,
					x: 2.15,
					y: -1.75,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				// Bottom side pins 17-24 (right to left)
				{
					pin: this.P17,
					x: 1.75,
					y: -2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P18,
					x: 1.25,
					y: -2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P19,
					x: 0.75,
					y: -2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P20,
					x: 0.25,
					y: -2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P21,
					x: -0.25,
					y: -2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P22,
					x: -0.75,
					y: -2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P23,
					x: -1.25,
					y: -2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P24,
					x: -1.75,
					y: -2.15,
					width: 0.2,
					height: 0.6,
					shape: PadShape.Rectangular
				},
				// Left side pins 25-32 (bottom to top)
				{
					pin: this.P25,
					x: -2.15,
					y: -1.75,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P26,
					x: -2.15,
					y: -1.25,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P27,
					x: -2.15,
					y: -0.75,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P28,
					x: -2.15,
					y: -0.25,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P29,
					x: -2.15,
					y: 0.25,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P30,
					x: -2.15,
					y: 0.75,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P31,
					x: -2.15,
					y: 1.25,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.P32,
					x: -2.15,
					y: 1.75,
					width: 0.6,
					height: 0.2,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-2.0, -2.0), new Vec2(2.0, -2.0), new Vec2(2.0, 2.0), new Vec2(-2.0, 2.0)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

/**
 * SOT‑23-6 DDC package footprint for TPS563201.
 * Pinout (top view):
 *   1: GND   2: SW   3: VIN
 *   4: VOUT (feedback, not used in this design)
 *   5: EN    6: VBST
 * Pads are 0.95 × 1.1 mm, pitch 0.95 mm, row spacing 1.1 mm.
 */
export class TPS563201 extends Entity {
	public readonly VIN = new Pin("VIN")
	public readonly OUT = new Pin("OUT")
	public readonly GND = new Pin("GND")
	public readonly SW = new Pin("SW")
	public readonly EN = new Pin("EN")
	public readonly VBST = new Pin("VBST")
	public readonly footprint: Footprint
	constructor() {
		super("TPS563201")
		this.footprint = new Footprint({
			name: "TPS563201",
			pads: [
				// Top row: pins 1-2-3 (left to right)
				{
					pin: this.GND,
					x: -0.95,
					y: +0.55,
					width: 0.95,
					height: 1.1,
					shape: PadShape.Rectangular
				},
				{
					pin: this.SW,
					x: 0.0,
					y: +0.55,
					width: 0.95,
					height: 1.1,
					shape: PadShape.Rectangular
				},
				{
					pin: this.VIN,
					x: +0.95,
					y: +0.55,
					width: 0.95,
					height: 1.1,
					shape: PadShape.Rectangular
				},
				// Bottom row: pins 6-5 (right to left), pin 4 (VOUT/FB) skipped
				{
					pin: this.VBST,
					x: -0.95,
					y: -0.55,
					width: 0.95,
					height: 1.1,
					shape: PadShape.Rectangular
				},
				{
					pin: this.EN,
					x: 0.0,
					y: -0.55,
					width: 0.95,
					height: 1.1,
					shape: PadShape.Rectangular
				}
				// Pin 4 (VOUT/FB) not used in this design; skip pad
			],
			points: [new Vec2(-1.45, -0.8), new Vec2(1.45, -0.8), new Vec2(1.45, 0.8), new Vec2(-1.45, 0.8)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class InductorL1 extends Entity {
	public readonly L1_IN = new Pin("L1_IN")
	public readonly L1_OUT = new Pin("L1_OUT")
	public readonly footprint: Footprint
	constructor() {
		super("InductorL1")
		this.footprint = new Footprint({
			name: "InductorL1_Conn2",
			pads: [
				{
					pin: this.L1_IN,
					x: -2.0,
					y: 0.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.L1_OUT,
					x: 2.0,
					y: 0.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-3.0, -1.0), new Vec2(3.0, -1.0), new Vec2(3.0, 1.0), new Vec2(-3.0, 1.0)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class ESP32_WROOM_32E extends Entity {
	/** 5 V power input from the onboard regulator */
	public readonly V5_IN = new Pin("5V_IN")
	/** 3.3 V supply output to the ESP32 module */
	public readonly V33 = new Pin("3V3")
	/** Ground */
	public readonly gnd = new Pin("GND")
	/** I²C SDA (GPIO21) */
	public readonly GPIO21 = new Pin("GPIO21")
	/** I²C SCL (GPIO22) */
	public readonly GPIO22 = new Pin("GPIO22")
	/** Module enable (reset enable, active high) */
	public readonly EN = new Pin("EN")
	/** GPIO36 / ADC1_CH0 input (SENSOR_VP) */
	public readonly SENSOR_VP = new Pin("SENSOR_VP")
	/** GPIO39 / ADC1_CH3 input (SENSOR_VN) */
	public readonly SENSOR_VN = new Pin("SENSOR_VN")
	/** GPIO34 input (GPIO34) */
	public readonly GPIO34 = new Pin("GPIO34")
	/** GPIO35 input (GPIO35) */
	public readonly GPIO35 = new Pin("GPIO35")
	/** GPIO32 / ADC1_CH4 (GPIO32) */
	public readonly GPIO32 = new Pin("GPIO32")
	/** GPIO33 / ADC1_CH5 (GPIO33) */
	public readonly GPIO33 = new Pin("GPIO33")
	/** GPIO25 / ADC2_CH8 / DAC1 (GPIO25) */
	public readonly GPIO25 = new Pin("GPIO25")
	/** GPIO26 / ADC2_CH9 / DAC2 (GPIO26) */
	public readonly GPIO26 = new Pin("GPIO26")
	/** GPIO27 / ADC2_CH7 (GPIO27) */
	public readonly GPIO27 = new Pin("GPIO27")
	/** GPIO14 / ADC2_CH6 (GPIO14) */
	public readonly GPIO14 = new Pin("GPIO14")
	/** GPIO12 / ADC2_CH5 (GPIO12) */
	public readonly GPIO12 = new Pin("GPIO12")
	/** GPIO13 / ADC2_CH4 (GPIO13) */
	public readonly GPIO13 = new Pin("GPIO13")
	/** GPIO15 / ADC2_CH3 (GPIO15) */
	public readonly GPIO15 = new Pin("GPIO15")
	/** GPIO2 / ADC2_CH2 (GPIO2) */
	public readonly GPIO2 = new Pin("GPIO2")
	/** GPIO0 / ADC2_CH1 (GPIO0, boot strapping) */
	public readonly GPIO0 = new Pin("GPIO0")
	/** GPIO4 / ADC2_CH0 (GPIO4) */
	public readonly GPIO4 = new Pin("GPIO4")
	/** GPIO16 (GPIO16, UART2_RX) */
	public readonly GPIO16 = new Pin("GPIO16")
	/** GPIO17 (GPIO17, UART2_TX) */
	public readonly GPIO17 = new Pin("GPIO17")
	/** GPIO5 / VSPI_CS0 (GPIO5) */
	public readonly GPIO5 = new Pin("GPIO5")
	/** GPIO18 / VSPI_CLK (GPIO18) */
	public readonly GPIO18 = new Pin("GPIO18")
	/** GPIO19 / VSPI_MISO (GPIO19) */
	public readonly GPIO19 = new Pin("GPIO19")
	/** GPIO23 / VSPI_MOSI (GPIO23) */
	public readonly GPIO23 = new Pin("GPIO23")
	/** GPIO22 / I2C_SCL (GPIO22, duplicate for completeness) */
	// public readonly IO22 = new Pin("IO22") // Already declared as GPIO22 above
	/** U0RXD / GPIO3 (RXD0, UART0 RX) */
	public readonly RXD0 = new Pin("RXD0")
	/** U0TXD / GPIO1 (TXD0, UART0 TX) */
	public readonly TXD0 = new Pin("TXD0")

	public readonly footprint: Footprint
	constructor() {
		super("ESP32-WROOM-32E")
		this.footprint = new Footprint({
			name: "ESP32-WROOM-32E",
			pads: [
				{
					pin: this.V5_IN,
					x: -9.0,
					y: 5.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.V33,
					x: -6.0,
					y: 5.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.GPIO21,
					x: -3.0,
					y: 5.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO22,
					x: -1.0,
					y: 5.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{ pin: this.EN, x: 1.0, y: 5.0, width: 1.0, height: 1.0, shape: PadShape.Circular },
				{
					pin: this.SENSOR_VP,
					x: 3.0,
					y: 5.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.SENSOR_VN,
					x: 5.0,
					y: 5.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO34,
					x: 7.0,
					y: 5.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO35,
					x: 9.0,
					y: 5.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO32,
					x: 9.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO33,
					x: 7.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO25,
					x: 5.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO26,
					x: 3.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO27,
					x: 1.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO14,
					x: -1.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO12,
					x: -3.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO13,
					x: -5.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO15,
					x: -7.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO2,
					x: -9.0,
					y: 3.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO0,
					x: -9.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO4,
					x: -7.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO16,
					x: -5.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO17,
					x: -3.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO5,
					x: -1.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO18,
					x: 1.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO19,
					x: 3.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.GPIO23,
					x: 5.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.RXD0,
					x: 7.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.TXD0,
					x: 9.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Circular
				},
				{
					pin: this.gnd,
					x: 9.0,
					y: -5.0,
					width: 2.0,
					height: 2.0,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-9.0, 5.0), new Vec2(-9.0, -5.0), new Vec2(9.0, -5.0), new Vec2(9.0, 5.0)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 3.0 }
		})
	}
}

/**
 * AMS1117-3.3 Linear Voltage Regulator (LDO) in SOT-223 package.
 * Steps down input voltage (up to 15 V) to a stable 3.3 V output.
 * Typical dropout voltage ~1.1 V @ 1 A.
 * Pinout (front view):
 *   1: GND       – Ground
 *   2: VOUT      – 3.3 V regulator output
 *   3: VIN       – Regulator input
 *   Tab: GND     – Heatsink tab tied to ground
 */
export class AMS1117_3_3 extends Entity {
	/**
	 * Regulator input pin. Accepts up to ~15 V (max spec).
	 */
	public readonly VIN = new Pin("VIN")

	/**
	 * Regulator output pin. Provides a regulated 3.3 V output.
	 */
	public readonly VOUT = new Pin("VOUT")

	/**
	 * Ground return pin. Also connected to the SOT-223 tab.
	 */
	public readonly GND = new Pin("GND")

	/**
	 * SOT-223 footprint for AMS1117-3.3:
	 * pins 1=GND, 2=VOUT, 3=VIN; tab on the back is also GND.
	 * Pads: 1.2 × 1.2 mm, tab pad: 5.0 × 1.5 mm.
	 */
	public readonly footprint: Footprint

	constructor() {
		super("AMS1117-3.3")
		this.footprint = new Footprint({
			name: "AMS1117-3.3_SOT-223",
			pads: [
				{
					pin: this.VIN,
					x: -2.0,
					y: 1.0,
					width: 1.2,
					height: 1.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.VOUT,
					x: 0.0,
					y: 1.0,
					width: 1.2,
					height: 1.2,
					shape: PadShape.Rectangular
				},
				{
					pin: this.GND,
					x: 2.0,
					y: 1.0,
					width: 1.2,
					height: 1.2,
					shape: PadShape.Rectangular
				},
				// Tab pad for GND
				{
					pin: this.GND,
					x: 0.0,
					y: -1.0,
					width: 5.0,
					height: 1.5,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-3.0, -1.5), new Vec2(3.0, -1.5), new Vec2(3.0, 2.0), new Vec2(-3.0, 2.0)],
			lineWidth: 0.15,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class OV2640 extends Entity {
	public readonly VCC = new Pin("VCC")
	public readonly SDA = new Pin("SDA")
	public readonly SCL = new Pin("SCL")
	/** Ground reference pin */
	public readonly GND = new Pin("GND")
	public readonly footprint: Footprint
	constructor() {
		super("OV2640")
		this.footprint = new Footprint({
			name: "OV2640_Module",
			pads: [
				{
					pin: this.VCC,
					x: -1.5,
					y: 1.5,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Rectangular
				},
				{
					pin: this.GND,
					x: -0.5,
					y: 1.5,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Rectangular
				},
				{
					pin: this.SDA,
					x: 0.5,
					y: 1.5,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Rectangular
				},
				{
					pin: this.SCL,
					x: 1.5,
					y: 1.5,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-2.0, -1.0), new Vec2(2.0, -1.0), new Vec2(2.0, 2.0), new Vec2(-2.0, 2.0)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class STUSB4500 extends Entity {
	/** Power supply input */
	public readonly VCC = new Pin("VCC")
	/** Ground reference pin */
	public readonly GND = new Pin("GND")
	public readonly SDA = new Pin("SDA")
	public readonly SCL = new Pin("SCL")
	public readonly footprint: Footprint
	constructor() {
		super("STUSB4500")
		this.footprint = new Footprint({
			name: "STUSB4500_QFN16",
			pads: [
				{
					pin: this.VCC,
					x: -1.0,
					y: 1.0,
					width: 0.5,
					height: 0.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.GND,
					x: 1.0,
					y: 1.0,
					width: 0.5,
					height: 0.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.SDA,
					x: -1.0,
					y: -1.0,
					width: 0.5,
					height: 0.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.SCL,
					x: 1.0,
					y: -1.0,
					width: 0.5,
					height: 0.5,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-1.5, -1.5), new Vec2(1.5, -1.5), new Vec2(1.5, 1.5), new Vec2(-1.5, 1.5)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class DCMotor extends Entity {
	public readonly IN1 = new Pin("IN1")
	public readonly IN2 = new Pin("IN2")
	public readonly footprint: Footprint
	constructor() {
		super("DCMotor")
		this.footprint = new Footprint({
			name: "DCMotor_Conn2",
			pads: [
				{
					pin: this.IN1,
					x: -2.0,
					y: 0.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Circular
				},
				{
					pin: this.IN2,
					x: 2.0,
					y: 0.0,
					width: 1.5,
					height: 1.5,
					shape: PadShape.Circular
				}
			],
			points: [new Vec2(-3.0, -1.0), new Vec2(3.0, -1.0), new Vec2(3.0, 1.0), new Vec2(-3.0, 1.0)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class DNF83x3 extends Footprint {
	public constructor(args: {
		pad1: Pin
		pad2: Pin
		pad3: Pin
		pad4: Pin
		pad5: Pin
		pad6: Pin
		pad7: Pin
		pad8: Pin
	}) {
		super({
			name: "DNF83x3",
			pads: [
				{
					pin: new Pin("VIN"),
					x: -1.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Rectangular
				},
				{
					pin: new Pin("VOUT"),
					x: 1.0,
					y: 1.0,
					width: 1.0,
					height: 1.0,
					shape: PadShape.Rectangular
				},
				{
					pin: new Pin("GND"),
					x: 0.0,
					y: -1.0,
					width: 2.0,
					height: 2.0,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-2.5, -2.5), new Vec2(2.5, -2.5), new Vec2(2.5, 2.5), new Vec2(-2.5, 2.5)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: -3.0 }
		})
	}
}

/**
 * DRV8876 H-Bridge Motor Driver component
 * Provides motor drive outputs with integrated current sensing and protection.
 */
export class DRV8876PWPR extends Entity {
	/** Sleep mode input: logic high to enable, low to enter low-power mode */
	public readonly nSLEEP = new Pin("nSLEEP")
	/** Current regulation and overcurrent protection mode select */
	public readonly IMODE = new Pin("IMODE")
	/** H-bridge control mode select (PH/EN, PWM, or independent half-bridge) */
	public readonly PMODE = new Pin("PMODE")
	/** H-bridge control input 1 */
	public readonly EN_IN1 = new Pin("EN/IN1")
	/** H-bridge control input 2 */
	public readonly PH_IN2 = new Pin("PH/IN2")
	/** Fault indicator open-drain output */
	public readonly nFAULT = new Pin("nFAULT")
	/** Proportional current output for current sensing */
	public readonly IPROPI = new Pin("IPROPI")
	/** Reference voltage input for current regulation threshold */
	public readonly VREF = new Pin("VREF")
	/** Power supply input (4.5–37 V) */
	public readonly VM = new Pin("VM")
	/** Power ground */
	public readonly PGND = new Pin("PGND")
	/** Ground reference pin */
	public readonly GND = new Pin("GND")
	/** H-bridge outputs to motor */
	public readonly OUT1 = new Pin("OUT1")
	/** H-bridge outputs to motor */
	public readonly OUT2 = new Pin("OUT2")
	/** Charge pump voltage output */
	public readonly VCP = new Pin("VCP")
	/** Charge pump switching node */
	public readonly CPH = new Pin("CPH")
	/** Charge pump flying capacitor node */
	public readonly CPL = new Pin("CPL")

	public readonly footprint: Footprint

	constructor() {
		super("DRV8876PWPR")
		this.footprint = new Footprint({
			name: "DRV8876_PWP_TSSOP16",
			pads: [
				// top side pins 1-8
				{
					pin: this.EN_IN1,
					x: -2.275,
					y: 2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.PH_IN2,
					x: -1.625,
					y: 2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.nSLEEP,
					x: -0.975,
					y: 2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.nFAULT,
					x: -0.325,
					y: 2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.VREF,
					x: 0.325,
					y: 2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.IPROPI,
					x: 0.975,
					y: 2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.IMODE,
					x: 1.625,
					y: 2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.OUT1,
					x: 2.275,
					y: 2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				// bottom side pins 16-9 (left to right)
				{
					pin: this.PMODE,
					x: -2.275,
					y: -2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.GND,
					x: -1.625,
					y: -2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.CPL,
					x: -0.975,
					y: -2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.CPH,
					x: -0.325,
					y: -2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.VCP,
					x: 0.325,
					y: -2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.VM,
					x: 0.975,
					y: -2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.OUT2,
					x: 1.625,
					y: -2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				},
				{
					pin: this.PGND,
					x: 2.275,
					y: -2.15,
					width: 0.3,
					height: 1.5,
					shape: PadShape.Rectangular
				}
			],
			points: [new Vec2(-2.75, -2.15), new Vec2(2.75, -2.15), new Vec2(2.75, 2.15), new Vec2(-2.75, 2.15)],
			lineWidth: 0.2,
			referenceOrigin: { x: 0, y: 0 }
		})
	}
}

export class MG995 extends Entity {
	public readonly VCC = new Pin("VCC")
	public readonly GND = new Pin("GND")
	public readonly PWM = new Pin("PWM")
	constructor() {
		super("MG995")
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
// Nets for two bidirectional motors (each needs its own H‑bridge)
const netMotor1A = new Net("MOTOR1_A")
const netMotor1B = new Net("MOTOR1_B")
const netMotor2A = new Net("MOTOR2_A")
const netMotor2B = new Net("MOTOR2_B")
// Control signals for motor driver 1
const netMD1_EN = new Net("MD1_EN")
const netMD1_PH = new Net("MD1_PH")
const netMD1_SLEEP = new Net("MD1_SLEEP")
// Control signals for motor driver 2
const netMD2_EN = new Net("MD2_EN")
const netMD2_PH = new Net("MD2_PH")
const netMD2_SLEEP = new Net("MD2_SLEEP")

// Servo motor control signal
const netServoSig = new Net("SERVO_SIG")

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

// Motor driver 1 and DC Motor 1 (bidirectional)
const motorDriver1 = new DRV8876PWPR()
netSystem12V.connect(motorDriver1.VM)
netGND.connect(motorDriver1.PGND)
netMotor1A.connect(motorDriver1.OUT1)
netMotor1B.connect(motorDriver1.OUT2)
// Motor driver 1 control connections
netMD1_EN.connect(motorDriver1.EN_IN1)
netMD1_PH.connect(motorDriver1.PH_IN2)
netMD1_SLEEP.connect(motorDriver1.nSLEEP)
// Map to ESP32 outputs
netMD1_EN.connect(esp.GPIO17)
netMD1_PH.connect(esp.GPIO16)
netMD1_SLEEP.connect(esp.GPIO4)
const motor1 = new DCMotor()
netMotor1A.connect(motor1.IN1)
netMotor1B.connect(motor1.IN2)

// Motor driver 2 and DC Motor 2 (bidirectional)
const motorDriver2 = new DRV8876PWPR()
netSystem12V.connect(motorDriver2.VM)
netGND.connect(motorDriver2.PGND)
netMotor2A.connect(motorDriver2.OUT1)
netMotor2B.connect(motorDriver2.OUT2)
// Motor driver 2 control connections
netMD2_EN.connect(motorDriver2.EN_IN1)
netMD2_PH.connect(motorDriver2.PH_IN2)
netMD2_SLEEP.connect(motorDriver2.nSLEEP)
// Map to ESP32 outputs
netMD2_EN.connect(esp.GPIO19)
netMD2_PH.connect(esp.GPIO18)
netMD2_SLEEP.connect(esp.GPIO13)
const motor2 = new DCMotor()
netMotor2A.connect(motor2.IN1)
netMotor2B.connect(motor2.IN2)

// Servo motor
const servo = new MG995()
net5V.connect(servo.VCC)
netGND.connect(servo.GND)
netServoSig.connect(servo.PWM)
// Map servo PWM to ESP32 pin
netServoSig.connect(esp.GPIO23)

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
		netI2C_SCL,
		netMotor1A,
		netMotor1B,
		netMotor2A,
		netMotor2B,
		netMD1_EN,
		netMD1_PH,
		netMD1_SLEEP,
		netMD2_EN,
		netMD2_PH,
		netMD2_SLEEP,
		netServoSig
	]
})

export const puppybotPCB = new PCB({
	name: "PuppyBot v0.2 PCB",
	material: "FR4",
	thickness: 1.6
})
// Add board layers
puppybotPCB.addLayer({ name: "TopCopper", type: "copper", material: "copper", thickness: 0.035 })
puppybotPCB.addLayer({
	name: "BottomCopper",
	type: "copper",
	material: "copper",
	thickness: 0.035
})
puppybotPCB.addLayer({
	name: "SilkscreenTop",
	type: "silkscreen",
	material: "silkscreen_ink",
	thickness: 0.1
})
puppybotPCB.addLayer({
	name: "SoldermaskTop",
	type: "soldermask",
	material: "epoxy_soldermask",
	thickness: 0.1
})

// Add components and nets to PCB
puppybotPCB.components.push(batteryPack, pptcFuse, tvsDiode, charger, buck, inductor, esp, ldo, camera, pdCtrl, motorDriver1, motor1, motorDriver2, motor2, servo)
puppybotPCB.nets.push(netBattery, netBattInput, netSystem12V, net5V, net3V3, netGND, netI2C_SDA, netI2C_SCL, netMotor1A, netMotor1B, netMotor2A, netMotor2B, netServoSig)
