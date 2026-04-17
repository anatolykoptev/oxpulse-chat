import { describe, it, expect } from 'vitest';
import { deriveFromHost, fromBackend } from './branding';

describe('deriveFromHost', () => {
	it('returns oxpulse as default partner_id', () => {
		const b = deriveFromHost();
		expect(b.partner_id).toBe('oxpulse');
	});

	it('returns non-empty title and site_name', () => {
		const b = deriveFromHost();
		expect(b.title).toBeTruthy();
		expect(b.site_name).toBeTruthy();
	});

	it('canonical and og_url end with a trailing slash', () => {
		const b = deriveFromHost();
		expect(b.canonical).toMatch(/\/$/);
		expect(b.og_url).toMatch(/\/$/);
	});

	it('affiliate is null for default', () => {
		const b = deriveFromHost();
		expect(b.affiliate).toBeNull();
	});
});

describe('fromBackend', () => {
	const raw = {
		partner_id: 'rvpn',
		domains: ['call.rvpn.online', 'www.rvpn.online'],
		display_name: 'RVPN Call',
		description: 'Private calls via RVPN',
		logo: { light: '/rvpn-light.svg', dark: '/rvpn-dark.svg' },
		favicon: '/rvpn-favicon.ico',
		og_image: '/rvpn-og.png',
		colors: { primary: '#FF6600', secondary: '#111827', accent: '#FFAA00' },
		copy: { hero_title_ru: 'Звонки' },
		affiliate: {
			vpn_cta_url: 'https://rvpn.online/buy',
			vpn_cta_text_ru: 'Купить VPN',
			vpn_cta_text_en: 'Get VPN',
		},
		legal: null,
	};

	it('maps partner_id correctly', () => {
		expect(fromBackend(raw).partner_id).toBe('rvpn');
	});

	it('maps display_name to both title and site_name', () => {
		const b = fromBackend(raw);
		expect(b.title).toBe('RVPN Call');
		expect(b.site_name).toBe('RVPN Call');
	});

	it('derives canonical from first domain', () => {
		expect(fromBackend(raw).canonical).toBe('https://call.rvpn.online/');
	});

	it('absolutizes relative og_image with first domain', () => {
		expect(fromBackend(raw).og_image).toBe('https://call.rvpn.online/rvpn-og.png');
	});

	it('passes through absolute og_image unchanged', () => {
		const b = fromBackend({ ...raw, og_image: 'https://cdn.example.com/img.png' });
		expect(b.og_image).toBe('https://cdn.example.com/img.png');
	});

	it('maps logo light and dark', () => {
		const b = fromBackend(raw);
		expect(b.logo_light).toBe('/rvpn-light.svg');
		expect(b.logo_dark).toBe('/rvpn-dark.svg');
	});

	it('maps colors correctly', () => {
		const b = fromBackend(raw);
		expect(b.primary_color).toBe('#FF6600');
		expect(b.accent_color).toBe('#FFAA00');
	});

	it('maps affiliate block', () => {
		const b = fromBackend(raw);
		expect(b.affiliate?.vpn_cta_url).toBe('https://rvpn.online/buy');
		expect(b.affiliate?.vpn_cta_text_ru).toBe('Купить VPN');
		expect(b.affiliate?.vpn_cta_text_en).toBe('Get VPN');
	});

	it('maps copy record', () => {
		expect(fromBackend(raw).copy).toEqual({ hero_title_ru: 'Звонки' });
	});

	it('uses fallback logo when logo field absent', () => {
		const b = fromBackend({ ...raw, logo: undefined });
		expect(b.logo_light).toBe('/logo-light.svg');
		expect(b.logo_dark).toBe('/logo-dark.svg');
	});

	it('uses fallback colors when colors field absent', () => {
		const b = fromBackend({ ...raw, colors: undefined });
		expect(b.primary_color).toBe('#0066FF');
		expect(b.accent_color).toBeNull();
	});

	// Store fetch behavior: unit-test helpers only; store integration test
	// skipped because Vitest lacks a jsdom environment wired to the SvelteKit
	// readable() lifecycle. Follow-up: add playwright e2e test for /api/branding
	// round-trip once the partner domain is live.
});
