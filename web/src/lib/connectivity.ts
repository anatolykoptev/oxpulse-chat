// Network resilience layer. Primary host (current page) is tried first;
// on network error (fetch throws, NOT on HTTP 4xx/5xx), falls back to the
// next mirror from /api/domains cache.
//
// LocalStorage schema:
//   "oxpulse:mirror_chain" = { version: number, chain: string[], fetchedAt: number }
//   TTL: 1 hour
// Chain is the full list with primary at index 0; iterated in order.

const CACHE_KEY = 'oxpulse:mirror_chain';
const CACHE_TTL_MS = 60 * 60 * 1000;

export interface DomainsResponse {
	primary: string;
	mirrors: string[];
	config_version: number;
}

interface CacheEntry {
	version: number;
	chain: string[];
	fetchedAt: number;
}

function readCache(): CacheEntry | null {
	if (typeof localStorage === 'undefined') return null;
	try {
		const raw = localStorage.getItem(CACHE_KEY);
		if (!raw) return null;
		const entry = JSON.parse(raw) as CacheEntry;
		if (!entry.chain || !Array.isArray(entry.chain)) return null;
		if (Date.now() - entry.fetchedAt > CACHE_TTL_MS) return null;
		return entry;
	} catch {
		return null;
	}
}

function writeCache(entry: CacheEntry): void {
	if (typeof localStorage === 'undefined') return;
	try {
		localStorage.setItem(CACHE_KEY, JSON.stringify(entry));
	} catch {
		// Quota exceeded or storage disabled — degrade silently.
	}
}

/**
 * Returns the fallback chain: [currentHost, ...mirrors].
 * If no cache and no fetch possible (SSR or offline), returns [currentHost].
 */
export function getMirrorChain(): string[] {
	if (typeof window === 'undefined') return [];
	const host = window.location.hostname;
	const cached = readCache();
	if (cached && cached.chain.length) return cached.chain;
	return [host];
}

/**
 * Fetches /api/domains and populates the cache. Non-fatal: on any error,
 * the previous cache stays (or absence remains).
 */
export async function refreshMirrorChain(): Promise<void> {
	if (typeof window === 'undefined') return;
	try {
		const r = await fetch('/api/domains', { credentials: 'same-origin' });
		if (!r.ok) throw new Error(`domains endpoint returned ${r.status}`);
		const data = (await r.json()) as DomainsResponse;
		const chain = [data.primary, ...data.mirrors];
		writeCache({ version: data.config_version, chain, fetchedAt: Date.now() });
	} catch (e) {
		console.warn('refreshMirrorChain failed:', e);
	}
}

/**
 * Fetches `path` trying primary host first, then each mirror. Relative paths
 * (starting with /) are prefixed with https://{host}{path}. Non-network
 * errors (HTTP 4xx/5xx) are NOT retried — only network errors
 * (fetch rejection / TypeError) trigger fallback.
 */
export async function fetchWithFallback(
	path: string,
	init?: RequestInit
): Promise<Response> {
	if (!path.startsWith('/')) {
		// Absolute URL or unsupported — delegate to plain fetch.
		return fetch(path, init);
	}

	const chain = getMirrorChain();
	if (chain.length === 0) {
		// No chain at all — plain fetch will use relative path against current origin.
		return fetch(path, init);
	}

	let lastErr: unknown = null;
	for (const host of chain) {
		const url = `https://${host}${path}`;
		try {
			return await fetch(url, init);
		} catch (e) {
			lastErr = e;
			// Try next mirror.
		}
	}
	throw lastErr ?? new Error('fetchWithFallback: all mirrors failed');
}
