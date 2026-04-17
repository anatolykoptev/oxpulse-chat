// Tests for fetchWithFallback (HTTP fallback logic).

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fetchWithFallback } from './connectivity';
import { CACHE_KEY, setupConnectivityEnv } from './connectivity.test-helpers';

beforeEach(() => setupConnectivityEnv(vi));
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

function seedChain(chain: string[]): void {
	localStorage.setItem(CACHE_KEY, JSON.stringify({ version: 1, chain, fetchedAt: Date.now() }));
}

describe('fetchWithFallback', () => {
	it('returns first successful response without trying mirrors', async () => {
		seedChain(['oxpulse.chat', 'call.piter.now', 'call.rvpn.online']);
		const mockFetch = vi.fn().mockResolvedValue({ ok: true, status: 200 } as Response);
		vi.stubGlobal('fetch', mockFetch);

		const resp = await fetchWithFallback('/api/branding');

		expect(resp.ok).toBe(true);
		expect(mockFetch).toHaveBeenCalledTimes(1);
		expect(mockFetch).toHaveBeenCalledWith('https://oxpulse.chat/api/branding', undefined);
	});

	it('falls back to next mirror on network error', async () => {
		seedChain(['oxpulse.chat', 'call.piter.now']);
		const mockFetch = vi
			.fn()
			.mockRejectedValueOnce(new TypeError('Failed to fetch'))
			.mockResolvedValueOnce({ ok: true, status: 200 } as Response);
		vi.stubGlobal('fetch', mockFetch);

		const resp = await fetchWithFallback('/api/turn-credentials');

		expect(resp.ok).toBe(true);
		expect(mockFetch).toHaveBeenCalledTimes(2);
		expect(mockFetch).toHaveBeenNthCalledWith(1, 'https://oxpulse.chat/api/turn-credentials', undefined);
		expect(mockFetch).toHaveBeenNthCalledWith(2, 'https://call.piter.now/api/turn-credentials', undefined);
	});

	it('does NOT retry on HTTP 5xx — returns the error response', async () => {
		seedChain(['oxpulse.chat', 'call.piter.now']);
		const errorResp = { ok: false, status: 503 } as Response;
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(errorResp));

		const resp = await fetchWithFallback('/api/event');

		expect(resp.status).toBe(503);
		// Only one call — no retry on HTTP errors.
		expect((globalThis.fetch as ReturnType<typeof vi.fn>)).toHaveBeenCalledTimes(1);
	});

	it('throws when all mirrors fail', async () => {
		seedChain(['oxpulse.chat', 'call.piter.now']);
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')));

		await expect(fetchWithFallback('/api/health')).rejects.toThrow();
	});

	it('delegates absolute URLs to plain fetch without modification', async () => {
		const mockFetch = vi.fn().mockResolvedValue({ ok: true, status: 200 } as Response);
		vi.stubGlobal('fetch', mockFetch);

		await fetchWithFallback('https://cdn.example.com/asset.js');

		expect(mockFetch).toHaveBeenCalledWith('https://cdn.example.com/asset.js', undefined);
	});
});
