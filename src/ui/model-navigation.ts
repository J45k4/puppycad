export type ModelNavigationNode = { key: string; label: string; children?: ModelNavigationNode[] }
export const navigationKey = (...values: (string | number)[]) => JSON.stringify(values)
export const readableName = (value: string) => value.replace(/[-_]+/g, " ").replace(/^./, (c) => c.toUpperCase())
