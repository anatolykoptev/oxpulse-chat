<script lang="ts">
  import type { Translations } from './i18n';

  let {
    localVideo = $bindable(),
    remoteVideo = $bindable(),
    remoteStream,
    localStream,
    videoEnabled,
    isPip,
    t,
  }: {
    localVideo: HTMLVideoElement;
    remoteVideo: HTMLVideoElement;
    remoteStream: MediaStream | null;
    localStream: MediaStream | null;
    videoEnabled: boolean;
    isPip: boolean;
    t: Translations;
  } = $props();
</script>

<!-- svelte-ignore a11y_media_has_caption -->
<video
  bind:this={remoteVideo}
  class="remote-video"
  class:visible={!!remoteStream}
  autoplay
  playsinline
  aria-label={t.peer}
></video>

<div class="local-container" class:pip={isPip}>
  <!-- svelte-ignore a11y_media_has_caption -->
  <video bind:this={localVideo} autoplay playsinline muted aria-label={t.you}></video>
  {#if !videoEnabled || !localStream}
    <div class="avatar-placeholder">
      <div class="avatar-circle">
        <svg viewBox="0 0 24 24"><path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
      </div>
    </div>
  {/if}
</div>

<div class="vignette"></div>

<style>
  .remote-video {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    z-index: 1;
    opacity: 0;
    transition: opacity 0.6s ease;
  }

  .remote-video.visible {
    opacity: 1;
  }

  .local-container {
    position: fixed;
    inset: 0;
    z-index: 2;
    overflow: hidden;
    background: #08080C;
  }

  .local-container video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .local-container.pip {
    inset: auto;
    bottom: 108px;
    right: 20px;
    width: 130px;
    height: 174px;
    z-index: 8;
    border-radius: 16px;
    border: 2px solid rgba(201, 169, 110, 0.25);
    box-shadow:
      0 8px 32px rgba(0, 0, 0, 0.5),
      0 0 0 1px rgba(0, 0, 0, 0.3);
    animation: morph-to-pip 0.65s cubic-bezier(0.4, 0, 0.2, 1);
  }

  @keyframes morph-to-pip {
    from {
      bottom: 0;
      right: 0;
      width: 100vw;
      height: 100dvh;
      border-radius: 0;
      border-color: transparent;
    }
  }

  .avatar-placeholder {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(8, 8, 12, 0.92);
  }

  .avatar-circle {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    background: var(--accent-dim);
    border: 1.5px solid rgba(201, 169, 110, 0.2);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .avatar-circle svg {
    width: 32px;
    height: 32px;
    stroke: var(--accent);
    fill: none;
    stroke-width: 1.5;
  }

  .pip .avatar-circle {
    width: 40px;
    height: 40px;
  }

  .pip .avatar-circle svg {
    width: 18px;
    height: 18px;
  }

  .vignette {
    position: fixed;
    inset: 0;
    z-index: 5;
    pointer-events: none;
    background: radial-gradient(
      ellipse at center,
      transparent 40%,
      rgba(8, 8, 12, 0.35) 100%
    );
  }

  @media (max-width: 640px) {
    .local-container.pip {
      width: 100px;
      height: 134px;
      bottom: 88px;
      right: 12px;
      border-radius: 12px;
    }
  }
</style>
