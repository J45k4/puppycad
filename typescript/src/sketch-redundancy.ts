/** Identify whole relations that add no local freedom restriction after earlier relations.
 * Two-pass orthogonalization avoids repeatedly reducing the full growing Jacobian.
 * Return no diagnostics when this analysis disagrees with the solver's rank.
 */
export function analyzeSketchJacobian(groups: { id: string; rows: number[][] }[], expectedRank: number, parameterCount: number): { redundantRelations: string[]; fixedParameters: boolean[] | null } {
	const basis: number[][] = []
	const redundant: string[] = []
	for (const group of groups) {
		const before = basis.length
		let active = false
		for (const row of group.rows) {
			const norm = Math.hypot(...row)
			if (norm < 1e-7) continue
			active = true
			const remainder = row.map((value) => value / norm)
			for (let pass = 0; pass < 2; pass++)
				for (const vector of basis) {
					const dot = remainder.reduce((sum, value, index) => sum + value * (vector[index] ?? 0), 0)
					for (let i = 0; i < remainder.length; i++) remainder[i] = (remainder[i] ?? 0) - dot * (vector[i] ?? 0)
				}
			const length = Math.hypot(...remainder)
			if (length > 1e-7) basis.push(remainder.map((value) => value / length))
		}
		if (active && basis.length === before) redundant.push(group.id)
	}
	if (basis.length !== expectedRank) return { redundantRelations: [], fixedParameters: null }
	const fixedParameters = Array.from({ length: parameterCount }, (_, column) => {
		const projection = basis.reduce((sum, row) => sum + (row[column] ?? 0) ** 2, 0)
		return Math.abs(1 - projection) < 1e-9
	})
	return { redundantRelations: redundant, fixedParameters }
}

export function redundantSketchRelations(groups: { id: string; rows: number[][] }[], expectedRank: number): string[] {
	return analyzeSketchJacobian(groups, expectedRank, 0).redundantRelations
}
