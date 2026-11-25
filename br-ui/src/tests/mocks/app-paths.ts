// Mock $app/paths for testing; routes resolve to themselves outside SvelteKit
export function resolve(path: string): string {
	return path;
}
