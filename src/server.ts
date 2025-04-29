import { promises as fs } from 'fs';
import { pathToFileURL } from 'url';
import path from 'path';
import index from "./index.html"
import { Schematic } from './puppycad';

/**
 * Dynamically loads all JS or TS modules from the given folder and returns their exports keyed by filename.
 */
export async function loadExportsFromFolder(folderPath: string): Promise<Record<string, any>> {
	const modules: Record<string, any> = {};
	const files = await fs.readdir(folderPath);
	for (const file of files) {
		const ext = file.split('.').pop();
		if (ext !== 'js' && ext !== 'ts') continue;
		const fullPath = path.join(folderPath, file);
		const fileUrl = pathToFileURL(fullPath).href;
		try {
			const mod = await import(fileUrl);
			modules[file] = mod;
		} catch (e) {
			console.error(`Failed to import ${file}:`, e);
		}
	}
	return modules;
}

Bun.serve({
	port: 5337,
	routes: {
		"/": index
	}
})

export const createPuppyCADServer = async (folderPath: string) => {
    // Load modules from the folder
    const modules = await loadExportsFromFolder(folderPath);

    // Flatten all named exports from each module into a single list
    const exportsList: any[] = [];
    for (const mod of Object.values(modules)) {
        for (const exported of Object.values(mod)) {
            exportsList.push(exported);
        }
    }

    // Log the flat list of exports
    // console.log("Flat list of all exports:", exportsList);

    // Optionally, identify Schematic instances
    exportsList.forEach((item) => {
        if (item instanceof Schematic) {
            console.log("Loaded Schematic instance:", item);
        }
    });

    return exportsList;
}