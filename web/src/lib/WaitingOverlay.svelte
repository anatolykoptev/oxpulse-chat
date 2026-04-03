<script lang="ts">
  import type { Translations } from './i18n';

  let {
    roomId,
    onShare,
    t,
  }: {
    roomId: string;
    onShare: () => void;
    t: Translations;
  } = $props();
</script>

<div class="waiting-overlay" role="status" aria-live="polite">
  <div class="waiting-content">
    <div class="pulse-container" aria-hidden="true">
      <div class="pulse-ring"></div>
      <div class="pulse-ring ring-2"></div>
      <div class="pulse-ring ring-3"></div>
      <div class="pulse-core"></div>
    </div>
    <span class="waiting-text">{t.waiting}</span>
    <div class="room-code-block">
      <span class="room-code-label">{t.roomCodeLabel}</span>
      <span class="room-code-value">{roomId}</span>
    </div>
    <button class="share-link-btn" onclick={onShare} title={t.copyLink} aria-label={t.copyLink}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71"/>
        <path d="M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71"/>
      </svg>
      <span>{t.copyLink}</span>
    </button>
  </div>
</div>

<style>
  .waiting-overlay {
    position: fixed;
    inset: 0;
    z-index: 6;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(8, 8, 12, 0.6);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    animation: fade-in 0.4s ease-out;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .waiting-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 28px;
  }

  .pulse-container {
    position: relative;
    width: 80px;
    height: 80px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .pulse-core {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 20px rgba(201, 169, 110, 0.4);
    animation: core-breathe 3s ease-in-out infinite;
  }

  @keyframes core-breathe {
    0%, 100% { transform: scale(1); opacity: 0.9; }
    50% { transform: scale(1.15); opacity: 1; }
  }

  .pulse-ring {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1px solid var(--accent);
    opacity: 0;
    animation: ring-expand 3.5s ease-out infinite;
  }

  .ring-2 { animation-delay: 1.15s; }
  .ring-3 { animation-delay: 2.3s; }

  @keyframes ring-expand {
    0% { transform: scale(0.3); opacity: 0.6; }
    100% { transform: scale(2.2); opacity: 0; }
  }

  .waiting-text {
    font-size: 15px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.7);
    letter-spacing: 0.8px;
    text-transform: uppercase;
  }

  .room-code-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .room-code-label {
    font-size: 11px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.55);
    text-transform: uppercase;
    letter-spacing: 1.5px;
  }

  .room-code-value {
    font-family: var(--mono);
    font-size: 28px;
    font-weight: 600;
    color: var(--accent);
    letter-spacing: 3px;
    padding: 8px 20px;
    background: rgba(201, 169, 110, 0.08);
    border: 1px solid rgba(201, 169, 110, 0.2);
    border-radius: 12px;
    user-select: all;
  }

  .share-link-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 24px;
    border-radius: 999px;
    border: 1px solid rgba(201, 169, 110, 0.3);
    background: rgba(201, 169, 110, 0.1);
    color: var(--accent);
    font-family: var(--font);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.25s ease;
  }

  .share-link-btn:hover {
    background: rgba(201, 169, 110, 0.18);
    border-color: rgba(201, 169, 110, 0.5);
    box-shadow: 0 0 24px rgba(201, 169, 110, 0.12);
    transform: scale(1.03);
  }

  .share-link-btn:active {
    transform: scale(0.97);
  }

  @media (max-width: 640px) {
    .waiting-text {
      font-size: 14px;
    }
  }
</style>
