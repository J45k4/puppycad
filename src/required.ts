/** Fail at the boundary instead of propagating an absent model reference. */
export function requireValue<T>(value: T | null | undefined): T {
	if (value === null || value === undefined) throw new Error("Required model value is missing.")
	return value
}
