<script lang="ts">
  import type { Translations } from './i18n';

  let { t }: { t: Translations } = $props();

  let infoOpen = $state(false);
</script>

<footer class="footer">
  <span class="footer-copy">&copy; 2026 OxPulse</span>
  <p class="oss">Open Source · <a href="https://github.com/anatolykoptev/oxpulse-chat" target="_blank" rel="noopener">GitHub</a></p>
  <nav class="footer-links desktop-only">
    <a href="/privacy">{t.footerPrivacy}</a>
    <a href="/terms">{t.footerTerms}</a>
    <a href="/accessibility">{t.footerAccessibility}</a>
    <a href="mailto:hi@oxpulse.ru">{t.footerContact}</a>
  </nav>
  <div class="info-btn-wrap mobile-only">
    <button class="info-btn" onclick={() => infoOpen = !infoOpen} aria-label="Info">?</button>
    {#if infoOpen}
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <div class="info-overlay" onclick={() => infoOpen = false}></div>
      <nav class="info-popup">
        <a href="/privacy" class="info-link">{t.footerPrivacy}</a>
        <a href="/terms" class="info-link">{t.footerTerms}</a>
        <a href="/accessibility" class="info-link">{t.footerAccessibility}</a>
        <a href="mailto:hi@oxpulse.ru" class="info-link">{t.footerContact}</a>
      </nav>
    {/if}
  </div>
</footer>

<style>
  .footer {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 3;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 14px 24px;
    font-family: var(--font);
    font-size: 13px;
    color: rgba(255, 255, 255, 0.55);
    animation: footer-in 1s cubic-bezier(0.16, 1, 0.3, 1) both;
    animation-delay: 0.8s;
  }

  @keyframes footer-in {
    from { opacity: 0; transform: translateY(24px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .footer-copy { white-space: nowrap; }

  .oss {
    margin: 0;
    white-space: nowrap;
    color: rgba(255, 255, 255, 0.55);
  }

  .oss a {
    color: rgba(255, 255, 255, 0.55);
    text-decoration: none;
    transition: color 0.2s ease;
  }

  .oss a:hover { color: var(--accent); }

  .footer-links {
    display: flex;
    gap: 16px;
  }

  .footer-links a {
    color: rgba(255, 255, 255, 0.55);
    text-decoration: none;
    transition: color 0.2s ease;
    white-space: nowrap;
  }

  .footer-links a:hover { color: var(--accent); }

  .mobile-only { display: none; }

  .info-btn-wrap { position: relative; }

  .info-btn {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.6);
    font-family: var(--font);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s ease;
    padding: 0;
  }

  .info-btn:active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .info-overlay {
    position: fixed;
    inset: 0;
    z-index: 99;
  }

  .info-popup {
    position: absolute;
    bottom: 40px;
    right: 0;
    z-index: 100;
    background: #161620;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 8px 0;
    min-width: 180px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    animation: popup-in 0.15s ease-out;
  }

  .info-link {
    display: block;
    padding: 10px 16px;
    color: rgba(255, 255, 255, 0.7);
    text-decoration: none;
    font-size: 14px;
    transition: background 0.15s ease;
    white-space: nowrap;
  }

  .info-link:active { background: rgba(255, 255, 255, 0.06); }

  @keyframes popup-in {
    from { opacity: 0; transform: translateY(8px) scale(0.95); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }

  @media (max-width: 600px) {
    .desktop-only { display: none; }
    .mobile-only { display: block; }

    .footer {
      position: relative;
      padding: 12px 24px;
      padding-bottom: max(12px, env(safe-area-inset-bottom, 12px));
    }
  }
</style>
