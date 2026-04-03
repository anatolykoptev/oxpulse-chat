<script lang="ts">
  import type { Translations } from './i18n';

  let {
    timerStr,
    elapsed,
    t,
    onNewCall,
    onGoHome,
    onResetCountdown,
    redirectCountdown,
  }: {
    timerStr: string;
    elapsed: number;
    t: Translations;
    onNewCall: () => void;
    onGoHome: () => void;
    onResetCountdown: () => void;
    redirectCountdown: number;
  } = $props();

  const ratingEmojis = ['😞', '😐', '🙂', '😊', '🤩'];
  let rating = $state(0);
  let ratingSubmitted = $state(false);

  function submitRating(value: number) {
    rating = value;
    ratingSubmitted = true;
    onResetCountdown();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
<div class="ended" onclick={onResetCountdown} onmousemove={onResetCountdown}>
  <div class="ended-icon-wrap">
    <div class="ended-ring">
      <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10.68 13.31a16 16 0 003.41 2.6l1.27-1.27a2 2 0 012.11-.45 12.84 12.84 0 004.73.89 2 2 0 012 2v3a2 2 0 01-2.18 2 19.79 19.79 0 01-8.63-3.07 19.5 19.5 0 01-6-6A19.79 19.79 0 014.02 4.18 2 2 0 016 2h3a2 2 0 012 1.72c.1.63.26 1.24.45 1.83a2 2 0 01-.45 2.11L9.73 8.93a16 16 0 00.95 4.38z"/>
        <line x1="23" y1="1" x2="1" y2="23"/>
      </svg>
    </div>
  </div>
  <span class="ended-title">{t.callEnded}</span>
  {#if elapsed > 0}
    <span class="ended-duration">{timerStr}</span>
  {/if}

  <!-- Rating -->
  <div class="rating-block" role="group" aria-label={t.rateCall}>
    {#if !ratingSubmitted}
      <span class="rating-prompt" id="rating-label">{t.rateCall}</span>
      <div class="rating-emojis" role="radiogroup" aria-labelledby="rating-label">
        {#each ratingEmojis as emoji, i}
          <button
            class="rating-btn"
            class:selected={rating === i + 1}
            onclick={() => submitRating(i + 1)}
            aria-label="{i + 1} / 5"
            role="radio"
            aria-checked={rating === i + 1}
          >
            {emoji}
          </button>
        {/each}
      </div>
    {:else}
      <span class="rating-thanks" role="status" aria-live="polite">{t.rateThanks}</span>
    {/if}
  </div>

  <!-- Actions -->
  <div class="ended-actions">
    <button class="ended-btn ended-btn-primary" onclick={onNewCall} aria-label={t.newCallBtn}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
      </svg>
      {t.newCallBtn}
    </button>
    <button class="ended-btn ended-btn-ghost" onclick={onGoHome} aria-label={t.goHome}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/>
      </svg>
      {t.goHome}
    </button>
  </div>

  <!-- Redirect countdown -->
  <div class="redirect-bar" role="timer" aria-live="off" aria-label="{t.redirecting} {redirectCountdown}s">
    <span class="redirect-text">{t.redirecting}</span>
    <div class="redirect-progress" role="progressbar" aria-valuenow={redirectCountdown} aria-valuemin={0} aria-valuemax={10}>
      <div class="redirect-fill" style="width: {redirectCountdown * 10}%"></div>
    </div>
  </div>
</div>

<style>
  .ended {
    height: 100dvh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 20px;
    font-family: var(--font);
    background: #08080C;
    animation: fade-in 0.5s ease-out;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .ended-icon-wrap {
    margin-bottom: 8px;
    animation: ended-appear 0.6s cubic-bezier(0.4, 0, 0.2, 1);
  }

  @keyframes ended-appear {
    from { opacity: 0; transform: scale(0.85); }
    to { opacity: 1; transform: scale(1); }
  }

  .ended-ring {
    width: 80px;
    height: 80px;
    border-radius: 50%;
    border: 1.5px solid rgba(201, 169, 110, 0.2);
    background: rgba(201, 169, 110, 0.06);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent);
    box-shadow: 0 0 40px rgba(201, 169, 110, 0.08);
  }

  .ended-title {
    font-size: 22px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.88);
    letter-spacing: -0.3px;
  }

  .ended-duration {
    font-family: var(--mono);
    font-size: 15px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.7);
    letter-spacing: 0.5px;
    padding: 6px 16px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .rating-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    margin-top: 8px;
    animation: fade-in 0.6s ease-out 0.3s both;
  }

  .rating-prompt {
    font-size: 16px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.75);
  }

  .rating-emojis {
    display: flex;
    gap: 8px;
  }

  .rating-btn {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    border: 1px solid var(--glass-border);
    background: rgba(255, 255, 255, 0.04);
    font-size: 22px;
    cursor: pointer;
    transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }

  .rating-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.15);
    transform: translateY(-3px) scale(1.1);
  }

  .rating-btn:active {
    transform: translateY(0) scale(0.95);
  }

  .rating-btn.selected {
    background: var(--accent-dim);
    border-color: rgba(201, 169, 110, 0.4);
    transform: scale(1.15);
    box-shadow: 0 0 20px rgba(201, 169, 110, 0.15);
  }

  .rating-thanks {
    font-size: 14px;
    font-weight: 500;
    color: var(--accent);
    animation: fade-in 0.4s ease-out;
  }

  .ended-actions {
    display: flex;
    gap: 12px;
    margin-top: 8px;
    animation: fade-in 0.6s ease-out 0.5s both;
  }

  .ended-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 24px;
    border-radius: 12px;
    font-family: var(--font);
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.25s ease;
    border: 1px solid var(--glass-border);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.85);
  }

  .ended-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.15);
    transform: translateY(-1px);
  }

  .ended-btn:active {
    transform: translateY(0) scale(0.98);
  }

  .ended-btn-primary {
    border-color: rgba(201, 169, 110, 0.25);
    background: rgba(201, 169, 110, 0.08);
    color: var(--accent);
  }

  .ended-btn-primary:hover {
    background: rgba(201, 169, 110, 0.14);
    border-color: rgba(201, 169, 110, 0.4);
  }

  .ended-btn-ghost {
    border-color: rgba(255, 255, 255, 0.1);
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
  }

  .ended-btn-ghost:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.85);
  }

  .redirect-bar {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
    animation: fade-in 0.6s ease-out 0.7s both;
  }

  .redirect-text {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.55);
  }

  .redirect-progress {
    width: 120px;
    height: 3px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.06);
    overflow: hidden;
  }

  .redirect-fill {
    height: 100%;
    border-radius: 2px;
    background: rgba(201, 169, 110, 0.35);
    transition: width 1s linear;
  }
</style>
