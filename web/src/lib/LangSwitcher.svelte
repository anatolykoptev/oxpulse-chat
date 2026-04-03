<script lang="ts">
  import { LOCALES, LOCALE_LABELS, locale, setLocale, type Locale } from './i18n';
</script>

<div class="lang-switcher" role="radiogroup" aria-label="Language">
  {#each LOCALES as l}
    <button
      class="lang-btn"
      class:active={$locale === l}
      onclick={() => setLocale(l)}
      aria-label={l === 'ru' ? 'Русский' : 'English'}
      aria-pressed={$locale === l}
    >
      {LOCALE_LABELS[l]}
    </button>
  {/each}
</div>

<style>
  .lang-switcher {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .lang-btn {
    padding: 6px 12px;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: rgba(255, 255, 255, 0.65);
    font-family: 'Onest', system-ui, sans-serif;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.5px;
    cursor: pointer;
    transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .lang-btn:hover {
    color: rgba(255, 255, 255, 0.7);
    background: rgba(255, 255, 255, 0.06);
  }

  .lang-btn.active {
    background: rgba(201, 169, 110, 0.18);
    color: #C9A96E;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.2);
  }
</style>
