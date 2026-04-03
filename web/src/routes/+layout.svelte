<script lang="ts">
	import { onMount } from 'svelte';
	import { initLocale } from '$lib/i18n';

	let { children } = $props();

	onMount(() => {
		initLocale();

		if ('serviceWorker' in navigator) {
			// Unregister old SW from /room/ path
			navigator.serviceWorker.getRegistrations().then((regs) => {
				for (const r of regs) {
					if (r.scope.includes('/room')) r.unregister();
				}
			});
			navigator.serviceWorker.register('/sw.js').then((reg) => {
				// When a new SW is waiting, tell it to activate immediately
				if (reg.waiting) reg.waiting.postMessage('skipWaiting');
				reg.addEventListener('updatefound', () => {
					const newSW = reg.installing;
					if (!newSW) return;
					newSW.addEventListener('statechange', () => {
						if (newSW.state === 'installed' && navigator.serviceWorker.controller) {
							newSW.postMessage('skipWaiting');
						}
					});
				});
			}).catch(() => {});
		}
	});
</script>

<svelte:head>
	<link rel="manifest" href="/manifest.json" />
	<link rel="icon" type="image/png" sizes="192x192" href="/icon-192.png" />
	<link rel="icon" type="image/png" sizes="512x512" href="/icon-512.png" />
	<meta name="theme-color" content="#08080C" />
	<meta name="mobile-web-app-capable" content="yes" />
	<meta name="apple-mobile-web-app-capable" content="yes" />
	<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent" />
	<meta name="apple-mobile-web-app-title" content="OxPulse" />
	<link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
</svelte:head>

{@render children()}

<style>
	:global(html), :global(body) {
		margin: 0;
		padding: 0;
		background: #06060A;
		color: rgba(255, 255, 255, 0.88);
	}
</style>
