// Tests for getMirrorChain and refreshMirrorChain (cache layer).

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { getMirrorChain, refreshMirrorChain, type DomainsResponse } from './connectivity';
import { CACHE_KEY, setupConnectivityEnv } from './connectivity.test-helpers';

beforeEach(() => setupConnectivityEnv(vi));
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe('getMirrorChain', () => {
	it('returns cached chain when cache is fresh', () => {
		const chain = ['oxpulse.chat', 'call.piter.now'];
		localStorage.setItem(CACHE_KEY, JSON.stringify({ version: 1, chain, fetchedAt: Date.now() }));
		expect(getMirrorChain()).toEqual(chain);
	});

	it('returns [currentHost] when no cache', () => {
		expect(getMirrorChain()).toEqual(['oxpulse.chat']);
	});

	it('returns [currentHost] when cache is expired', () => {
		const chain = ['oxpulse.chat', 'call.piter.now'];
		const oldTime = Date.now() - 2 * 60 * 60 * 1000;
		localStorage.setItem(CACHE_KEY, JSON.stringify({ version: 1, chain, fetchedAt: oldTime }));
		expect(getMirrorChain()).toEqual(['oxpulse.chat']);
	});

	it('returns [currentHost] when cache entry is malformed', () => {
		localStorage.setItem(CACHE_KEY, 'not-json');
		expect(getMirrorChain()).toEqual(['oxpulse.chat']);
	});
});

describe('refreshMirrorChain', () => {
	it('writes cache on successful fetch', async () => {
		const data: DomainsResponse = {
			primary: 'oxpulse.chat',
			mirrors: ['call.piter.now', 'call.rvpn.online'],
			config_version: 1
		};
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => data }));

		await refreshMirrorChain();

		const raw = localStorage.getItem(CACHE_KEY);
		expect(raw).not.toBeNull();
		const entry = JSON.parse(raw!);
		expect(entry.chain).toEqual(['oxpulse.chat', 'call.piter.now', 'call.rvpn.online']);
		expect(entry.version).toBe(1);
	});

	it('leaves cache untouched when fetch rejects', async () => {
		const chain = ['oxpulse.chat', 'old-mirror.example.com'];
		localStorage.setItem(CACHE_KEY, JSON.stringify({ version: 1, chain, fetchedAt: Date.now() }));
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('network down')));

		await expect(refreshMirrorChain()).resolves.toBeUndefined();

		const entry = JSON.parse(localStorage.getItem(CACHE_KEY)!);
		expect(entry.chain).toEqual(chain);
	});

	it('leaves cache untouched when endpoint returns non-ok status', async () => {
		const chain = ['oxpulse.chat'];
		localStorage.setItem(CACHE_KEY, JSON.stringify({ version: 1, chain, fetchedAt: Date.now() }));
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 503 }));

		await refreshMirrorChain();

		const entry = JSON.parse(localStorage.getItem(CACHE_KEY)!);
		expect(entry.chain).toEqual(chain);
	});
});
