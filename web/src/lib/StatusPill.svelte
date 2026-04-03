<script lang="ts">
  import type { Translations } from './i18n';
  import type { QualityLevel } from './webrtc';

  let {
    timerStr,
    quality,
    verificationEmoji = '',
    t,
  }: {
    timerStr: string;
    quality: QualityLevel;
    verificationEmoji?: string;
    t: Translations;
  } = $props();
</script>

<div class="status-pill">
  <span class="live-dot"></span>
  <span class="timer-text">{timerStr}</span>
  <span class="quality-badge quality-{quality}" aria-label="Connection quality: {quality}">
    <svg width="14" height="12" viewBox="0 0 14 12" aria-hidden="true">
      <rect x="0" y="8" width="3" height="4" rx="1" fill="currentColor" />
      <rect x="5" y="4" width="3" height="8" rx="1" fill="currentColor" opacity={quality === 'poor' ? 0.2 : 1} />
      <rect x="10" y="0" width="3" height="12" rx="1" fill="currentColor" opacity={quality === 'good' ? 1 : 0.2} />
    </svg>
  </span>
  {#if verificationEmoji}
    <span class="e2ee-badge" title="{t.verifyCode}: {verificationEmoji}">
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0110 0v4"/>
      </svg>
      <span class="e2ee-emoji">{verificationEmoji}</span>
    </span>
  {/if}
</div>

<style>
  .status-pill {
    position: fixed;
    top: 20px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 18px;
    background: var(--glass);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border-radius: 999px;
    border: 1px solid var(--glass-border);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
    animation: slide-down 0.4s ease-out;
  }

  @keyframes slide-down {
    from { opacity: 0; transform: translateX(-50%) translateY(-12px); }
    to { opacity: 1; transform: translateX(-50%) translateY(0); }
  }

  .live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 8px rgba(74, 222, 128, 0.5);
    animation: live-pulse 2.5s ease-in-out infinite;
  }

  @keyframes live-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.45; }
  }

  .timer-text {
    font-family: var(--mono);
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.8);
    letter-spacing: 0.5px;
    font-variant-numeric: tabular-nums;
  }

  .quality-badge {
    display: flex;
    align-items: center;
    transition: color 0.3s ease;
  }

  .quality-good { color: var(--success); }
  .quality-fair { color: #FBBF24; }
  .quality-poor { color: var(--danger); }

  .e2ee-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--success);
    font-size: 11px;
    padding-left: 6px;
    border-left: 1px solid var(--glass-border);
    cursor: help;
  }

  .e2ee-emoji {
    font-size: 10px;
    letter-spacing: 1px;
  }

  @media (max-width: 640px) {
    .status-pill {
      top: 14px;
      padding: 6px 14px;
    }

    .timer-text {
      font-size: 12px;
    }
  }
</style>
