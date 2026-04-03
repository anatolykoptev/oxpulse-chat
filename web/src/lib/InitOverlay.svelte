<script lang="ts">
  import type { Translations } from './i18n';

  let {
    status,
    t,
  }: {
    status: 'init' | 'failed';
    t: Translations;
  } = $props();
</script>

{#if status === 'init'}
  <div class="overlay" role="status" aria-live="polite">
    <div class="loader" aria-hidden="true"></div>
    <span class="text">{t.connecting}</span>
  </div>
{:else}
  <div class="overlay" role="alert">
    <svg class="fail-icon" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>
    </svg>
    <span class="text fail-text">{t.failed}</span>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 20px;
    background: rgba(8, 8, 12, 0.92);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    animation: fade-in 0.3s ease-out;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .loader {
    width: 36px;
    height: 36px;
    border: 2px solid rgba(255, 255, 255, 0.08);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .text {
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.7);
    letter-spacing: 0.8px;
    text-transform: uppercase;
  }

  .fail-icon {
    color: var(--danger);
  }

  .fail-text {
    color: var(--danger) !important;
  }
</style>
