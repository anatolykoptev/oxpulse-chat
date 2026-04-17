<script lang="ts">
  import type { Translations } from './i18n';
  import { branding } from '$lib/branding';
  import { locale } from '$lib/i18n';
  import Logo from '$lib/Logo.svelte';

  let { t }: { t: Translations } = $props();

  const heroTitle = $derived(
    $branding.copy[`hero_title_${$locale}`] ?? t.heroTitle
  );
</script>

<header class="hero">
  <div class="logo-mark">
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="3" fill="var(--accent)" />
      <circle cx="12" cy="12" r="7" stroke="var(--accent)" stroke-width="1" opacity="0.4" />
      <circle cx="12" cy="12" r="11" stroke="var(--accent)" stroke-width="0.5" opacity="0.2" />
    </svg>
    <span class="logo-text">{$branding.site_name}</span>
    {#if $branding.co_brand_partner}
      <span class="co-brand-partner">× {$branding.co_brand_partner}</span>
    {/if}
  </div>

  <h1 class="hero-title">
    <span class="title-line">{heroTitle}</span>
    <span class="title-accent">{t.heroTitleAccent}</span>
  </h1>

  <p class="hero-sub">{t.heroSub}</p>
</header>

<style>
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: clamp(8px, 2dvh, 20px);
    animation: hero-in 1s cubic-bezier(0.16, 1, 0.3, 1) both;
    animation-delay: 0.15s;
  }

  @keyframes hero-in {
    from { opacity: 0; transform: translateY(24px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .logo-mark {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 4px;
  }

  .logo-text {
    font-family: var(--font);
    font-weight: 700;
    font-size: 15px;
    letter-spacing: 1.8px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.9);
  }

  .co-brand-partner {
    font-family: var(--font);
    font-weight: 500;
    font-size: 11px;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.6);
    margin-left: 4px;
  }

  .hero-title {
    margin: 0;
    text-align: center;
    line-height: 1.05;
    font-size: clamp(36px, 7vw, 56px);
    font-weight: 400;
    letter-spacing: -1.5px;
    color: #fff;
  }

  .title-line {
    display: block;
    font-family: var(--font);
  }

  .title-accent {
    display: block;
    font-family: var(--serif);
    font-style: italic;
    color: var(--accent);
    font-size: clamp(42px, 9vw, 68px);
    margin-top: -2px;
  }

  .hero-sub {
    margin: 0;
    font-size: 16px;
    font-weight: 400;
    color: rgba(255, 255, 255, 0.88);
    text-align: center;
    line-height: 1.5;
    max-width: 320px;
    letter-spacing: 0.15px;
  }

  @media (max-height: 680px) {
    .hero-sub { display: none; }
    .logo-mark { margin-bottom: 0; }
  }
</style>
