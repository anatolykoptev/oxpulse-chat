import { readable, type Readable } from 'svelte/store';

export interface BrandingData {
	partner_id: string;
	title: string;
	site_name: string;
	description: string;
	canonical: string;
	og_url: string;
	og_image: string;
	favicon: string;
	logo_light: string;
	logo_dark: string;
	primary_color: string;
	secondary_color: string;
	accent_color: string | null;
	copy: Record<string, string>;
	affiliate: {
		vpn_cta_url: string;
		vpn_cta_text_ru: string;
		vpn_cta_text_en: string;
	} | null;
}

export function deriveFromHost(): BrandingData {
	// Browser fallback before /api/branding resolves — uses location.host
	// so the initial render is not empty. The fetch then fills in.
	const host = typeof window !== 'undefined' ? window.location.hostname : 'oxpulse.chat';
	return {
		partner_id: 'oxpulse',
		title: 'OxPulse',
		site_name: 'OxPulse',
		description: 'Secure, free video calls',
		canonical: `https://${host}/`,
		og_url: `https://${host}/`,
		og_image: `https://${host}/og-image.png`,
		favicon: '/favicon.svg',
		logo_light: '/logo-light.svg',
		logo_dark: '/logo-dark.svg',
		primary_color: '#0066FF',
		secondary_color: '#1E293B',
		accent_color: null,
		copy: {},
		affiliate: null,
	};
}

export function fromBackend(raw: Record<string, unknown>): BrandingData {
	const firstDomain = (raw.domains as string[] | undefined)?.[0] ?? 'oxpulse.chat';
	const rawOgImage = raw.og_image as string | undefined ?? '';
	const logo = raw.logo as { light?: string; dark?: string } | undefined;
	const colors = raw.colors as { primary?: string; secondary?: string; accent?: string } | undefined;
	const affiliate = raw.affiliate as BrandingData['affiliate'];
	return {
		partner_id: raw.partner_id as string,
		title: raw.display_name as string,
		site_name: raw.display_name as string,
		description: raw.description as string,
		canonical: `https://${firstDomain}/`,
		og_url: `https://${firstDomain}/`,
		og_image: rawOgImage.startsWith('http')
			? rawOgImage
			: `https://${firstDomain}${rawOgImage}`,
		favicon: raw.favicon as string,
		logo_light: logo?.light ?? '/logo-light.svg',
		logo_dark: logo?.dark ?? '/logo-dark.svg',
		primary_color: colors?.primary ?? '#0066FF',
		secondary_color: colors?.secondary ?? '#1E293B',
		accent_color: colors?.accent ?? null,
		copy: (raw.copy as Record<string, string>) ?? {},
		affiliate: affiliate ?? null,
	};
}

export const branding: Readable<BrandingData> = readable(deriveFromHost(), (set) => {
	if (typeof window === 'undefined') return;
	fetch('/api/branding', { credentials: 'same-origin' })
		.then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
		.then((raw) => set(fromBackend(raw)))
		.catch((e) => {
			// Non-fatal: keep the host-derived fallback. Log to console only.
			console.warn('branding fetch failed:', e);
		});
});
