<script lang="ts">
  import type { Translations } from './i18n';
  import CodeInput from './CodeInput.svelte';

  let {
    t,
    roomCode = $bindable(''),
    codeError = '',
    onCreateRoom,
    onJoinRoom,
  }: {
    t: Translations;
    roomCode: string;
    codeError: string;
    onCreateRoom: () => void;
    onJoinRoom: () => void;
  } = $props();
</script>

<div class="actions-block" role="group" aria-label={t.newCall}>
  <button class="btn-primary" onclick={onCreateRoom} aria-label={t.newCall}>
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M22 16.92v3a2 2 0 01-2.18 2 19.79 19.79 0 01-8.63-3.07 19.5 19.5 0 01-6-6 19.79 19.79 0 01-3.07-8.67A2 2 0 014.11 2h3a2 2 0 012 1.72c.1.63.26 1.25.45 1.85a2 2 0 01-.45 2.11L8.09 8.7a16 16 0 006 6l1.27-1.27a2 2 0 012.11-.45c.6.19 1.22.36 1.85.45A2 2 0 0122 16.92z"/>
      <line x1="12" y1="1" x2="12" y2="7"/>
      <line x1="9" y1="4" x2="15" y2="4"/>
    </svg>
    <span>{t.newCall}</span>
  </button>

  <div class="divider-row" aria-hidden="true">
    <span class="divider-line"></span>
    <span class="divider-text">{t.or}</span>
    <span class="divider-line"></span>
  </div>

  <CodeInput bind:value={roomCode} onsubmit={onJoinRoom} />

  <button
    class="btn-join"
    onclick={onJoinRoom}
    disabled={roomCode.replace(/[^A-Z0-9]/gi, '').length < 8}
    aria-label={t.joinCall}
  >
    {t.joinCall}
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <line x1="5" y1="12" x2="19" y2="12"/>
      <polyline points="12 5 19 12 12 19"/>
    </svg>
  </button>
  {#if codeError}
    <span class="error-text" id="code-error" role="alert">{codeError}</span>
  {/if}
</div>

<style>
  .actions-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: clamp(12px, 2dvh, 20px);
    width: 100%;
    max-width: 380px;
    animation: actions-in 1s cubic-bezier(0.16, 1, 0.3, 1) both;
    animation-delay: 0.35s;
  }

  @keyframes actions-in {
    from { opacity: 0; transform: translateY(20px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .btn-primary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    width: 100%;
    padding: clamp(14px, 2dvh, 18px) 28px;
    border: 1.5px solid rgba(201, 169, 110, 0.5);
    border-radius: 14px;
    background: linear-gradient(135deg, rgba(201, 169, 110, 0.2) 0%, rgba(201, 169, 110, 0.1) 100%);
    color: #fff;
    font-family: var(--font);
    font-size: 17px;
    font-weight: 600;
    letter-spacing: 0.3px;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
    box-shadow: 0 0 24px rgba(201, 169, 110, 0.08), inset 0 1px 0 rgba(255, 255, 255, 0.06);
  }

  .btn-primary:hover {
    border-color: rgba(201, 169, 110, 0.7);
    background: linear-gradient(135deg, rgba(201, 169, 110, 0.28) 0%, rgba(201, 169, 110, 0.14) 100%);
    box-shadow:
      0 0 40px rgba(201, 169, 110, 0.15),
      0 8px 32px rgba(0, 0, 0, 0.2),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
    transform: translateY(-1px);
  }

  .btn-primary:active {
    transform: translateY(0) scale(0.98);
    background: linear-gradient(135deg, rgba(201, 169, 110, 0.24) 0%, rgba(201, 169, 110, 0.12) 100%);
  }

  .divider-row {
    display: flex;
    align-items: center;
    gap: 16px;
    width: 100%;
  }

  .divider-line {
    flex: 1;
    height: 1px;
    background: rgba(255, 255, 255, 0.1);
  }

  .divider-text {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.6);
    text-transform: uppercase;
    letter-spacing: 2px;
  }

  .btn-join {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: clamp(10px, 1.5dvh, 14px) 20px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.95);
    font-family: var(--font);
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.25s ease;
    white-space: nowrap;
  }

  .btn-join:hover:not(:disabled) {
    border-color: rgba(255, 255, 255, 0.22);
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
  }

  .btn-join:active:not(:disabled) {
    transform: scale(0.97);
  }

  .btn-join:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .error-text {
    font-size: 14px;
    color: var(--danger);
    font-weight: 500;
    animation: shake 0.4s ease;
  }

  @keyframes shake {
    0%, 100% { transform: translateX(0); }
    25% { transform: translateX(-4px); }
    75% { transform: translateX(4px); }
  }
</style>
