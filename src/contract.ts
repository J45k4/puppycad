export type ContractSchema = "puppycad.contract@0.1"

export interface PuppyCadContract {
	schema: ContractSchema
	id: `contract.${string}` // globally unique
	version: string // semver of THIS contract document
	publishedAt: string // ISO timestamp
	publisher: Publisher // who signs it
	scope: "device" | "line" | "facility" | "company"
	name: string
	description?: string
	visibility: "public" | "private" | "customer-only"
	regions?: string[] // ISO country/region codes served
	certifications?: string[] // ISO9001, AS9100, CE, RoHS, ITAR, etc.

	processes: ProcessCapability[] // CNC, 3DP, Injection, PCB-FAB, PCB-ASM, SheetMetal...

	materials?: MaterialCatalogItem[] // optional global list
	logistics?: LogisticsPolicy
	pricing?: PricingPolicy // base pricing model
	leadTimes?: LeadTimePolicy // nominal lead times per process/material/tier
	quality?: QualityPolicy
	quoting?: QuotingAPI // how to quote
	ordering?: OrderingAPI // how to place/track orders
	webhooks?: WebhookSpec[] // status callbacks

	dfmDrivers: DFMDriverRef[] // pointers to rule packs (validation & transforms)
	transforms?: TransformRef[] // optional rewrite/optimization drivers

	trust?: TrustTelemetry // rolling KPIs (for score computation)
	terms?: CommercialTerms // SLAs, refund policy, IP/confidentiality

	signatures?: Signature[] // detached signatures (publisher, optionally PuppyCad)
	meta?: Record<string, unknown> // vendor-defined extras
}

export interface Publisher {
	legalName: string
	duns?: string
	vatId?: string
	contact: { email: string; phone?: string; url?: string }
	address?: string
}

export type ProcessKind = "CNC" | "Additive:FDM" | "Additive:SLA" | "Additive:SLS" | "InjectionMolding" | "SheetMetal" | "PCB:Fabrication" | "PCB:Assembly"

export interface ProcessCapability {
	kind: ProcessKind
	name?: string // e.g., "Haas VF-2", "Prusa MK4 Farm #3"
	deviceIds?: string[] // serials/asset tags if scope=device
	capacity?: Capacity // throughput, build-envelope, lanes
	constraints: ConstraintSet[] // geometric & process constraints
	materials?: MaterialCatalogItem[] // supported materials for this process
	tolerances?: ToleranceSpec // general capability tolerances
	finishes?: string[] // anodize types, bead blast, etc.
	costing?: PricingPolicy // overrides
	leadTimes?: LeadTimePolicy // overrides
	dfmDrivers?: DFMDriverRef[] // overrides/extends global
}

export interface Capacity {
	envelope?: { x: number; y: number; z: number; units: "mm" | "in" }
	batch?: { parallelJobs?: number; perDay?: number }
	materialsParallel?: number
}

export interface ConstraintSet {
	id: string
	appliesTo: ("part" | "feature" | "material" | "assembly")[]
	// Expression DSL or structured rules; simple v0 shape:
	rules: Rule[]
}

export type Rule =
	| { code: "thin_wall"; min: number; units: "mm" }
	| { code: "min_hole_dia"; min: number; units: "mm" }
	| { code: "max_part_size"; x: number; y: number; z: number; units: "mm" }
	| { code: "draft_angle_min"; minDeg: number }
	| { code: "overhang_max"; deg: number } // for 3DP
	| { code: "no_internal_undercut" } // CNC
	| { code: "trace_width_min"; min: number; units: "mm" } // PCB
	| { code: "clearance_min"; min: number; units: "mm" } // PCB
	| { code: "via_drill_min"; min: number; units: "mm" } // PCB
	| { code: "layers_max"; max: number } // PCB
	| { code: string; [k: string]: unknown } // extensible

export interface MaterialCatalogItem {
	code: string // e.g., "AL6061-T6", "PLA", "FR-4"
	kind: "metal" | "polymer" | "composite" | "pcb" | "resin" | "other"
	properties?: Record<string, number | string>
	finishes?: string[]
}

export interface ToleranceSpec {
	general?: string // e.g., "ISO 2768-mK"
	linear?: { plusMinus: number; units: "mm" }
	hole?: { h7?: boolean; h9?: boolean; custom?: string }
	flatness?: { value: number; units: "mm" }
	surfaceFinishRa?: { value: number; units: "µm" }
}

export interface LogisticsPolicy {
	incoterms?: string[] // EXW, FOB, DDP...
	shipFrom?: string[] // regions/addresses
	shipMethods?: string[] // DHL, UPS, sea, air
}

export interface PricingPolicy {
	model: "table" | "formula" | "quote-only"
	currency?: string
	// v0 simple fields; can be per-process override
	baseSetupFee?: number
	perPart?: number // simple FDM example
	perVolumeCm3?: number // 3DP variable cost
	cncPerHour?: number // CNC example
	discounts?: { moq: number; percentOff: number }[]
}

export interface LeadTimePolicy {
	standardDays?: number
	expediteDays?: number
	expediteMultiplier?: number
	notes?: string
}

export interface QualityPolicy {
	inspections?: ("visual" | "dimensional" | "CMM" | "Xray" | "AOI")[]
	sampling?: string // e.g., "ANSI/ASQ Z1.4 AQL 1.0"
	reports?: ("material-cert" | "fpv" | "cofc" | "ppap")[]
}

export interface DFMDriverRef {
	id: `driver.${string}` // unique name in registry
	version: string // semver
	uri?: string // where to fetch (if public)
	visibility?: "public" | "private" | "customer-only"
	// Optional inline minimal rules for portability:
	inlineRules?: ConstraintSet[]
}

export interface TransformRef {
	id: `transform.${string}`
	version: string
	purpose: "3DP->Injection" | "3DP->CNC" | "CostReduce" | "SplitForMachining" | string
	uri?: string
}

export interface QuotingAPI {
	mode: "api" | "email" | "portal"
	endpoint?: string // HTTPS endpoint if mode=api
	auth?: { kind: "apikey" | "oauth2" | "signed-url"; doc?: string }
	supportsBatch?: boolean
	// Minimal quote request/response shapes (opaque pass-through allowed):
	requestSchemaRef?: string
	responseSchemaRef?: string
}

export interface OrderingAPI {
	mode: "api" | "email" | "portal"
	endpoint?: string
	auth?: { kind: "apikey" | "oauth2" | "signed-url"; doc?: string }
	requiresQuoteId?: boolean
	statusPoll?: { endpoint?: string; intervalSec?: number }
	cancellationWindowHours?: number
}

export interface WebhookSpec {
	event: "quote.created" | "order.accepted" | "order.in_production" | "order.shipped" | "order.delivered" | string
	url: string
	secret?: string
}

export interface TrustTelemetry {
	windowDays: number // e.g., last 90 days
	onTimeDeliveryPct?: number
	rejectionRatePct?: number
	avgResponseHours?: number
	avgDeviationDays?: number
	customerNps?: number
	lastAudit?: string // ISO date
}

export interface CommercialTerms {
	ndaRequired?: boolean
	ipRetention: "customer" | "supplier" | "joint"
	warrantyDays?: number
	refundPolicy?: string
	sla?: { responseHours?: number; remakePolicy?: string }
}

export interface Signature {
	by: "publisher" | "puppycad" | string
	alg: "ed25519" | "secp256k1" | "rsa-pss"
	sig: string // base64url
	keyId?: string
	signedAt: string // ISO
}
