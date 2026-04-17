<script lang="ts">
  import { goto } from '$app/navigation';
  import { t } from '$lib/i18n';
  import { track } from '$lib/tracker';
  import { branding } from '$lib/branding';
  import LangSwitcher from '$lib/LangSwitcher.svelte';
  import { generateRoomCode, isValidRoomId } from '$lib/roomcode';
  import BackgroundLayer from '$lib/BackgroundLayer.svelte';
  import SignalPulse from '$lib/SignalPulse.svelte';
  import HeroSection from '$lib/HeroSection.svelte';
  import ActionsBlock from '$lib/ActionsBlock.svelte';
  import FeatureCards from '$lib/FeatureCards.svelte';
  import LandingFooter from '$lib/LandingFooter.svelte';
  import AffiliateBanner from '$lib/AffiliateBanner.svelte';

  let roomCode = $state('');
  let codeError = $state('');
  let mounted = $state(false);

  const pulseIntensity = $derived(() => {
    const clean = roomCode.replace(/[^A-Z0-9]/gi, '');
    return clean.length > 0 ? Math.min(clean.length / 8, 1) : 0;
  });

  $effect(() => {
    mounted = true;
    track('page_view', undefined, { referrer: document.referrer || '' });
    return () => { mounted = false; };
  });

  function createRoom() {
    const roomId = generateRoomCode();
    track('room_created', roomId);
    goto(`/${roomId}`, { replaceState: true });
  }

  function joinRoom() {
    const code = roomCode.trim();
    if (!code) return;
    const normalized = code.toUpperCase();
    if (isValidRoomId(normalized) || /^[a-zA-Z0-9_-]{4,}$/.test(code)) {
      codeError = '';
      goto(`/${normalized}`, { replaceState: true });
    } else {
      codeError = $t.invalidCode;
    }
  }
</script>

<svelte:head>
  <title>{$branding.title}</title>
  <meta name="description" content={$branding.description} />
  <link rel="canonical" href={$branding.canonical} />

  <!-- Open Graph -->
  <meta property="og:type" content="website" />
  <meta property="og:url" content={$branding.og_url} />
  <meta property="og:title" content={$branding.title} />
  <meta property="og:description" content={$branding.description} />
  <meta property="og:image" content={$branding.og_image} />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:site_name" content={$branding.site_name} />
  <meta property="og:locale" content="en_US" />
  <meta property="og:locale:alternate" content="ru_RU" />

  <!-- Twitter Card -->
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={$branding.title} />
  <meta name="twitter:description" content={$branding.description} />
  <meta name="twitter:image" content={$branding.og_image} />
</svelte:head>

<div class="landing" class:mounted>
  <div class="lang-corner">
    <LangSwitcher />
  </div>

  <BackgroundLayer />
  <SignalPulse intensity={pulseIntensity()} />

  <main class="content">
    <HeroSection t={$t} />

    <ActionsBlock
      t={$t}
      bind:roomCode
      {codeError}
      onCreateRoom={createRoom}
      onJoinRoom={joinRoom}
    />

    <FeatureCards t={$t} />
    <AffiliateBanner />
  </main>

  <LandingFooter t={$t} />
  <div class="bottom-vignette"></div>
</div>

<style>
  :root {
    --accent: var(--brand-primary, #C9A96E);
    --accent-dim: color-mix(in srgb, var(--brand-primary, #C9A96E) 15%, transparent);
    --accent-glow: color-mix(in srgb, var(--brand-primary, #C9A96E) 8%, transparent);
    --surface: rgba(255, 255, 255, 0.07);
    --surface-hover: rgba(255, 255, 255, 0.11);
    --border: rgba(255, 255, 255, 0.12);
    --border-accent: color-mix(in srgb, var(--brand-primary, #C9A96E) 30%, transparent);
    --font: 'Onest', system-ui, sans-serif;
    --mono: 'Martian Mono', 'JetBrains Mono', monospace;
    --serif: 'Cormorant Garamond', Georgia, serif;
    --danger: #FB7185;
  }

  .landing {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-family: var(--font);
    overflow: hidden;
    overscroll-behavior: none;
    opacity: 0;
    transition: opacity 0.8s ease;
    height: 100dvh;
    padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
  }

  .landing.mounted {
    opacity: 1;
  }

  .lang-corner {
    position: fixed;
    top: max(12px, env(safe-area-inset-top, 12px));
    right: max(16px, env(safe-area-inset-right, 16px));
    z-index: 10;
    animation: fade-in 1s cubic-bezier(0.16, 1, 0.3, 1) both;
    animation-delay: 0.6s;
  }

  @keyframes fade-in {
    from { opacity: 0; transform: translateY(24px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .content {
    position: relative;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: clamp(20px, 4dvh, 48px);
    padding: 24px;
    max-width: 480px;
    width: 100%;
  }

  .bottom-vignette {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    height: 120px;
    background: linear-gradient(to top, #06060A 0%, transparent 100%);
    z-index: 1;
    pointer-events: none;
  }

  @media (max-width: 480px) {
    .bottom-vignette { display: none; }
    .content { padding: 16px; }
  }
</style>
