<script lang="ts">
  import LangSwitcher from '$lib/LangSwitcher.svelte';

  let { title, children } = $props<{ title: string; children: any }>();
</script>

<svelte:head>
  <title>{title} — OxPulse</title>
</svelte:head>

<div class="legal-page">
  <header class="legal-header">
    <a href="/" class="back-link" aria-label="Back to home">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor"
        stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <line x1="19" y1="12" x2="5" y2="12"/>
        <polyline points="12 19 5 12 12 5"/>
      </svg>
    </a>
    <h1 class="legal-title">{title}</h1>
    <div class="lang-corner">
      <LangSwitcher />
    </div>
  </header>

  <main class="legal-body">
    {@render children()}
  </main>
</div>

<style>
  .legal-page {
    min-height: 100vh;
    background: #06060A;
    color: rgba(255, 255, 255, 0.82);
    font-family: var(--font, 'Onest', system-ui, sans-serif);
    padding: 0 24px 64px;
  }

  .legal-header {
    display: flex;
    align-items: center;
    gap: 16px;
    max-width: 720px;
    margin: 0 auto;
    padding: 28px 0 24px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .back-link {
    color: rgba(255, 255, 255, 0.5);
    text-decoration: none;
    display: flex;
    align-items: center;
    transition: color 0.2s ease;
    flex-shrink: 0;
  }

  .back-link:hover {
    color: #C9A96E;
  }

  .legal-title {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.92);
    flex: 1;
  }

  .lang-corner {
    flex-shrink: 0;
  }

  .legal-body {
    max-width: 720px;
    margin: 0 auto;
    padding-top: 32px;
    font-size: 16px;
    line-height: 1.7;
  }

  .legal-body :global(h2) {
    font-size: 18px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    margin: 32px 0 12px;
  }

  .legal-body :global(p) {
    margin: 0 0 16px;
    color: rgba(255, 255, 255, 0.72);
  }

  .legal-body :global(ul) {
    padding-left: 24px;
    margin: 0 0 16px;
  }

  .legal-body :global(li) {
    margin-bottom: 6px;
    color: rgba(255, 255, 255, 0.72);
  }

  .legal-body :global(a) {
    color: #C9A96E;
    text-decoration: none;
    transition: opacity 0.2s ease;
  }

  .legal-body :global(a:hover) {
    opacity: 0.8;
  }

  .legal-body :global(.legal-date) {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.4);
    margin-top: 40px;
  }
</style>
