// Shared test utilities for connectivity tests.
// Not imported by production code.

export const CACHE_KEY = 'oxpulse:mirror_chain';

/** Minimal in-memory localStorage stub for Node/Vitest environments. */
export function makeLocalStorageStub(): Storage {
	const store: Record<string, string> = {};
	return {
		getItem: (key: string) => store[key] ?? null,
		setItem: (key: string, value: string) => {
			store[key] = value;
		},
		removeItem: (key: string) => {
			delete store[key];
		},
		clear: () => {
			for (const k of Object.keys(store)) delete store[k];
		},
		get length() {
			return Object.keys(store).length;
		},
		key: (index: number) => Object.keys(store)[index] ?? null
	} as Storage;
}

/** Wire window + localStorage stubs before each test. */
export function setupConnectivityEnv(vi: typeof import('vitest')['vi']): void {
	const ls = makeLocalStorageStub();
	vi.stubGlobal('localStorage', ls);
	vi.stubGlobal('window', { location: { hostname: 'oxpulse.chat' }, localStorage: ls });
}
