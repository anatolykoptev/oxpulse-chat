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
	/// Optional co-brand partner name. Null for the default OxPulse brand;
	/// populated (e.g. "RVPN") on partner mirrors for "× Partner" mark and
	/// "Powered by Partner" footer credit.
	co_brand_partner: string | null;
	/// Optional canonical URL override. When set, `canonical` and `og_url`
	/// are set to this value instead of the per-host domain — used for
	/// unified-brand SEO consolidation to oxpulse.chat.
	canonical_override: string | null;
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
		co_brand_partner: null,
		canonical_override: null,
	};
}

/// Parse raw backend JSON (from /api/branding or bootstrap script tag) into
/// BrandingData. Throws if required fields are missing or not strings, so the
/// caller's try/catch can keep the last-known-good value instead of rendering
/// "undefined" in the document title.
export function fromBackend(raw: Record<string, unknown>): BrandingData {
	const partnerId = raw.partner_id;
	if (typeof partnerId !== 'string' || partnerId === '') {
		throw new Error(`branding: missing or empty partner_id in response`);
	}
	const displayName = raw.display_name;
	if (typeof displayName !== 'string' || displayName === '') {
		throw new Error(`branding: missing or empty display_name in response`);
	}
	const firstDomain = (raw.domains as string[] | undefined)?.[0] ?? 'oxpulse.chat';
	const rawOgImage = raw.og_image as string | undefined ?? '';
	const logo = raw.logo as { light?: string; dark?: string } | undefined;
	const colors = raw.colors as { primary?: string; secondary?: string; accent?: string } | undefined;
	const affiliate = raw.affiliate as BrandingData['affiliate'];
	// Unified-brand SEO: if the backend set canonical_override (partner mirrors
	// → https://oxpulse.chat/), use it for both canonical and og_url so that
	// client-hydrated <svelte:head> matches the SSR-rendered meta tags. Without
	// this, the <link rel="canonical"> silently flips back to the partner
	// domain on hydration and undoes the SEO consolidation.
	const canonicalOverrideRaw = raw.canonical_override;
	const canonicalOverride =
		typeof canonicalOverrideRaw === 'string' && canonicalOverrideRaw !== ''
			? canonicalOverrideRaw
			: null;
	const canonical = canonicalOverride ?? `https://${firstDomain}/`;
	const coBrandRaw = raw.co_brand_partner;
	const coBrand =
		typeof coBrandRaw === 'string' && coBrandRaw !== '' ? coBrandRaw : null;
	return {
		partner_id: partnerId,
		title: displayName,
		site_name: displayName,
		description: raw.description as string,
		canonical,
		og_url: canonical,
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
		co_brand_partner: coBrand,
		canonical_override: canonicalOverride,
	};
}

/// Read branding bootstrapped inline by the server into the `__branding_boot__`
/// script tag. Returns null in SSR context or if the tag is absent/unparseable.
function readBootstrap(): BrandingData | null {
	if (typeof document === 'undefined') return null;
	const el = document.getElementById('__branding_boot__');
	if (!el || !el.textContent) return null;
	try {
		const raw = JSON.parse(el.textContent);
		return fromBackend(raw);
	} catch (e) {
		console.warn('branding bootstrap parse failed:', e);
		return null;
	}
}

const initial: BrandingData = readBootstrap() ?? deriveFromHost();

export const branding: Readable<BrandingData> = readable(initial, (set) => {
	// Only fetch if bootstrap wasn't available — usually we already have it.
	// Still fetch for defense-in-depth (e.g., if the server was under heavy
	// load and skipped injection for some reason).
	if (typeof window === 'undefined') return;
	fetch('/api/branding', { credentials: 'same-origin' })
		.then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
		.then((raw) => {
			try {
				set(fromBackend(raw));
			} catch (e) {
				console.warn('branding parse failed:', e);
			}
		})
		.catch((e) => {
			// Non-fatal: keep the host-derived or bootstrap value. Log to console only.
			console.warn('branding fetch failed:', e);
		});
});
