<script lang="ts">
  import type { Translations } from './i18n';

  let { audioMuted, videoEnabled, speakerOn, screenSharing, onToggleAudio, onToggleVideo, onHangup, onShare, onFlipCamera, canFlipCamera, onToggleSpeaker, onToggleScreenShare, isMobile, t }: {
    audioMuted: boolean;
    videoEnabled: boolean;
    speakerOn?: boolean;
    screenSharing?: boolean;
    onToggleAudio: () => void;
    onToggleVideo: () => void;
    onHangup: () => void;
    onShare: () => void;
    onFlipCamera?: () => void;
    canFlipCamera?: boolean;
    onToggleSpeaker?: () => void;
    onToggleScreenShare?: () => void;
    isMobile?: boolean;
    t: Translations;
  } = $props();
</script>

<div class="controls" role="toolbar" aria-label="Call controls">
  <button
    class="ctrl-btn"
    class:muted={audioMuted}
    title={audioMuted ? t.unmute : t.mute}
    aria-label={audioMuted ? t.unmute : t.mute}
    aria-pressed={audioMuted}
    onclick={onToggleAudio}
  >
    {#if audioMuted}
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="1" y1="1" x2="23" y2="23"/><path d="M9 9v3a3 3 0 005.12 2.12M15 9.34V4a3 3 0 00-5.94-.6"/>
        <path d="M17 16.95A7 7 0 015 12"/><path d="M19 12a7 7 0 00-.11-1.23"/>
        <line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/>
      </svg>
    {:else}
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 1a3 3 0 00-3 3v8a3 3 0 006 0V4a3 3 0 00-3-3z"/>
        <path d="M19 10v2a7 7 0 01-14 0v-2"/>
        <line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/>
      </svg>
    {/if}
  </button>

  <button class="ctrl-btn hangup" title={t.hangup} aria-label={t.hangup} onclick={onHangup}>
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M10.68 13.31a16 16 0 003.41 2.6l1.27-1.27a2 2 0 012.11-.45 12.84 12.84 0 004.73.89 2 2 0 012 2v3a2 2 0 01-2.18 2 19.79 19.79 0 01-8.63-3.07 19.5 19.5 0 01-6-6A19.79 19.79 0 014.02 4.18 2 2 0 016 2h3a2 2 0 012 1.72c.1.63.26 1.24.45 1.83a2 2 0 01-.45 2.11L9.73 8.93a16 16 0 00.95 4.38z"/>
      <line x1="23" y1="1" x2="1" y2="23"/>
    </svg>
  </button>

  <button
    class="ctrl-btn"
    class:muted={!videoEnabled}
    title={videoEnabled ? t.disableVideo : t.enableVideo}
    aria-label={videoEnabled ? t.disableVideo : t.enableVideo}
    aria-pressed={!videoEnabled}
    onclick={onToggleVideo}
  >
    {#if videoEnabled}
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/>
      </svg>
    {:else}
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M16.5 7.5L23 7v10l-6.5-.5"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/>
        <line x1="1" y1="1" x2="23" y2="23"/>
      </svg>
    {/if}
  </button>

  {#if canFlipCamera && videoEnabled}
    <button class="ctrl-btn flip-btn" title="Flip camera" aria-label="Flip camera" onclick={onFlipCamera}>
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M11 19H4a2 2 0 01-2-2V7a2 2 0 012-2h5"/>
        <path d="M13 5h7a2 2 0 012 2v10a2 2 0 01-2 2h-5"/>
        <polyline points="16 3 19 6 16 9"/>
        <polyline points="8 15 5 18 8 21"/>
      </svg>
    </button>
  {/if}

  {#if !isMobile && onToggleScreenShare}
    <button
      class="ctrl-btn"
      class:screen-active={screenSharing}
      title={screenSharing ? t.stopShareScreen : t.shareScreen}
      aria-label={screenSharing ? t.stopShareScreen : t.shareScreen}
      aria-pressed={screenSharing}
      onclick={onToggleScreenShare}
    >
      {#if screenSharing}
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>
          <line x1="2" y1="2" x2="22" y2="22"/>
        </svg>
      {:else}
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>
        </svg>
      {/if}
    </button>
  {/if}

  {#if isMobile && onToggleSpeaker}
    <button class="ctrl-btn" class:speaker-off={!speakerOn} title={speakerOn ? 'Earpiece' : 'Speaker'} aria-label={speakerOn ? 'Earpiece' : 'Speaker'} aria-pressed={speakerOn} onclick={onToggleSpeaker}>
      {#if speakerOn}
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
          <path d="M15.54 8.46a5 5 0 010 7.07"/>
          <path d="M19.07 4.93a10 10 0 010 14.14"/>
        </svg>
      {:else}
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
          <path d="M15.54 8.46a5 5 0 010 7.07"/>
        </svg>
      {/if}
    </button>
  {/if}

  <div class="divider"></div>

  <button class="ctrl-btn share-icon-btn" title={t.share} aria-label={t.share} onclick={onShare}>
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71"/>
      <path d="M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71"/>
    </svg>
  </button>
</div>

<style>
  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    background: rgba(8, 8, 12, 0.55);
    backdrop-filter: blur(32px);
    -webkit-backdrop-filter: blur(32px);
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.07);
    box-shadow:
      0 8px 32px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.04);
  }

  .ctrl-btn {
    position: relative;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.07);
    color: rgba(255, 255, 255, 0.9);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .ctrl-btn:hover {
    background: rgba(255, 255, 255, 0.14);
    border-color: rgba(255, 255, 255, 0.18);
    transform: scale(1.06);
  }

  .ctrl-btn:active {
    transform: scale(0.95);
  }

  .ctrl-btn:focus-visible {
    outline: 2px solid var(--accent, #C9A96E);
    outline-offset: 2px;
  }

  .ctrl-btn.muted {
    color: rgba(255, 255, 255, 0.35);
    border-color: rgba(248, 113, 113, 0.25);
    background: rgba(248, 113, 113, 0.08);
  }

  .ctrl-btn.muted:hover {
    background: rgba(248, 113, 113, 0.15);
    border-color: rgba(248, 113, 113, 0.35);
  }

  .ctrl-btn.speaker-off {
    color: rgba(255, 255, 255, 0.5);
    border-color: rgba(255, 255, 255, 0.15);
  }

  .ctrl-btn.screen-active {
    color: var(--accent, #C9A96E);
    border-color: rgba(201, 169, 110, 0.4);
    background: rgba(201, 169, 110, 0.12);
  }

  .ctrl-btn.screen-active:hover {
    background: rgba(201, 169, 110, 0.2);
    border-color: rgba(201, 169, 110, 0.55);
  }

  .ctrl-btn.hangup {
    width: 52px;
    height: 52px;
    background: rgba(239, 68, 68, 0.65);
    border-color: rgba(239, 68, 68, 0.35);
    color: #fff;
  }

  .ctrl-btn.hangup:hover {
    background: rgba(220, 38, 38, 0.8);
    border-color: rgba(239, 68, 68, 0.5);
    box-shadow: 0 0 24px rgba(239, 68, 68, 0.25);
  }

  .divider {
    width: 1px;
    height: 28px;
    background: rgba(255, 255, 255, 0.08);
    margin: 0 4px;
    flex-shrink: 0;
  }

  .share-icon-btn {
    color: var(--accent, #C9A96E);
    border-color: rgba(201, 169, 110, 0.25);
    background: rgba(201, 169, 110, 0.08);
  }

  .share-icon-btn:hover {
    background: rgba(201, 169, 110, 0.16);
    border-color: rgba(201, 169, 110, 0.45);
    box-shadow: 0 0 20px rgba(201, 169, 110, 0.12);
  }
</style>
