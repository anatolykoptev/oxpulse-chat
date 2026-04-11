<script lang="ts">
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { onDestroy } from 'svelte';
  import { useCall } from '$lib/useCall.svelte';
  import CallControls from '$lib/CallControls.svelte';
  import StatusPill from '$lib/StatusPill.svelte';
  import VideoDisplay from '$lib/VideoDisplay.svelte';
  import WaitingOverlay from '$lib/WaitingOverlay.svelte';
  import EndedOverlay from '$lib/EndedOverlay.svelte';
  import InitOverlay from '$lib/InitOverlay.svelte';
  import Toast from '$lib/Toast.svelte';
  import { t } from '$lib/i18n';

  const roomId = $derived(page.params.roomId);
  const serverUrl = $derived(typeof window !== 'undefined' ? window.location.origin : '');
  const referrerUrl = $derived(typeof window !== 'undefined'
    ? new URLSearchParams(window.location.search).get('ref') || ''
    : '');

  let localVideo: HTMLVideoElement;
  let remoteVideo: HTMLVideoElement;
  let toastVisible = $state(false);
  let toastTimeout: ReturnType<typeof setTimeout> | null = null;
  let redirectCountdown = $state(10);
  let redirectTimer: ReturnType<typeof setInterval> | null = null;

  const call = useCall({
    get serverUrl() { return serverUrl; },
    get roomId() { return roomId; },
    onEnded() {
      if (referrerUrl) {
        window.location.href = referrerUrl;
      } else {
        startRedirectCountdown();
      }
    },
    getRemoteVideo() { return remoteVideo; },
  });

  // ── Redirect countdown ────────────────────────────────────────

  function startRedirectCountdown() {
    redirectCountdown = 10;
    redirectTimer = setInterval(() => {
      redirectCountdown -= 1;
      if (redirectCountdown <= 0) {
        if (redirectTimer) clearInterval(redirectTimer);
        goto('/', { replaceState: true });
      }
    }, 1000);
  }

  function resetRedirectCountdown() {
    redirectCountdown = 10;
  }

  function newCall() {
    if (redirectTimer) clearInterval(redirectTimer);
    goto(`/${crypto.randomUUID()}`, { replaceState: true });
  }

  // ── Toast ─────────────────────────────────────────────────────

  function share() {
    call.shareLink().then(() => {
      toastVisible = true;
      if (toastTimeout) clearTimeout(toastTimeout);
      toastTimeout = setTimeout(() => { toastVisible = false; }, 2200);
    });
  }

  // ── Bind video elements ───────────────────────────────────────

  $effect(() => { if (localVideo && call.localStream) localVideo.srcObject = call.localStream; });
  $effect(() => {
    if (remoteVideo && call.remoteStream) {
      remoteVideo.srcObject = call.remoteStream;
      // Listen for PiP exit via native browser controls
      remoteVideo.addEventListener('leavepictureinpicture', call.handlePipExit);
      return () => remoteVideo.removeEventListener('leavepictureinpicture', call.handlePipExit);
    }
  });

  // ── Lifecycle ─────────────────────────────────────────────────

  $effect(() => {
    if (roomId && serverUrl) call.init();

    window.addEventListener('beforeunload', call.beforeUnload);
    document.addEventListener('visibilitychange', call.handleVisibilityChange);

    return () => {
      window.removeEventListener('beforeunload', call.beforeUnload);
      document.removeEventListener('visibilitychange', call.handleVisibilityChange);
      call.destroy();
    };
  });

  onDestroy(() => {
    if (toastTimeout) clearTimeout(toastTimeout);
    if (redirectTimer) clearInterval(redirectTimer);
  });
</script>

<svelte:head>
  <title>OxPulse — Video Call</title>
  <meta name="theme-color" content="#08080C" />
  <meta name="robots" content="noindex, nofollow" />
  <meta property="og:type" content="website" />
  <meta property="og:title" content="OxPulse — Join Video Call" />
  <meta property="og:description" content="Join a private encrypted video call on OxPulse." />
  <meta property="og:image" content="https://oxpulse.chat/icon-512.png" />
  <meta name="twitter:card" content="summary" />
  <meta name="twitter:title" content="OxPulse — Join Video Call" />
  <meta name="twitter:description" content="Join a private encrypted video call on OxPulse." />
  <meta name="twitter:image" content="https://oxpulse.chat/icon-512.png" />
</svelte:head>

{#if call.status === 'ended'}
  <EndedOverlay
    timerStr={call.timerStr}
    elapsed={call.elapsed}
    t={$t}
    onNewCall={newCall}
    onGoHome={() => goto('/', { replaceState: true })}
    onResetCountdown={resetRedirectCountdown}
    {redirectCountdown}
  />
{:else}
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
  <div class="room" onmousemove={call.showControls} onclick={call.showControls}>
    <VideoDisplay
      bind:localVideo
      bind:remoteVideo
      remoteStream={call.remoteStream}
      localStream={call.localStream}
      videoEnabled={call.videoEnabled}
      isPip={call.isLocalPip}
      t={$t}
    />

    <!-- HUD: status + controls -->
    <div class="hud" class:hud-hidden={call.status === 'connected' && !call.controlsVisible}>
      {#if call.status === 'connected'}
        <StatusPill
          timerStr={call.timerStr}
          quality={call.quality}
          verificationEmoji={call.verificationEmoji}
          t={$t}
        />
      {/if}

      <div class="controls-dock">
        <CallControls
          audioMuted={call.audioMuted}
          videoEnabled={call.videoEnabled}
          speakerOn={call.speakerOn}
          screenSharing={call.screenSharing}
          onToggleAudio={call.toggleAudio}
          onToggleVideo={call.toggleVideo}
          onHangup={call.hangup}
          onShare={share}
          onFlipCamera={() => call.flipCamera(localVideo)}
          canFlipCamera={call.canFlipCamera}
          onToggleSpeaker={() => call.toggleSpeaker(remoteVideo)}
          onToggleScreenShare={call.toggleScreenShare}
          isMobile={call.isMobile}
          t={$t}
        />
      </div>
    </div>

    {#if call.status === 'waiting'}
      <WaitingOverlay {roomId} onShare={share} t={$t} />
    {/if}

    {#if call.status === 'init' || call.status === 'failed'}
      <InitOverlay status={call.status} t={$t} />
    {/if}

    <Toast visible={toastVisible} message={$t.copied} />
  </div>
{/if}

<style>
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    height: 100%;
    overflow: hidden;
    overscroll-behavior: none;
    background: #08080C;
    color: rgba(255, 255, 255, 0.88);
  }

  :root {
    --accent: #C9A96E;
    --accent-dim: rgba(201, 169, 110, 0.15);
    --success: #4ADE80;
    --danger: #FB7185;
    --glass: rgba(8, 8, 12, 0.55);
    --glass-border: rgba(255, 255, 255, 0.07);
    --font: 'Onest', system-ui, sans-serif;
    --mono: 'Martian Mono', 'JetBrains Mono', monospace;
  }

  .room {
    position: relative;
    width: 100vw;
    height: 100dvh;
    background: #08080C;
    font-family: var(--font);
    overflow: hidden;
    animation: fade-in 0.6s ease-out;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .hud {
    position: fixed;
    inset: 0;
    z-index: 10;
    pointer-events: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    padding-bottom: 28px;
    transition: opacity 0.4s ease;
  }

  .hud > * {
    pointer-events: auto;
  }

  .hud-hidden {
    opacity: 0;
    pointer-events: none !important;
  }

  .hud-hidden > * {
    pointer-events: none;
  }

  @media (hover: none) {
    .hud-hidden {
      opacity: 1;
      pointer-events: none;
    }
    .hud-hidden > * {
      pointer-events: auto;
    }
  }

  .controls-dock {
    animation: slide-up 0.4s ease-out;
  }

  @keyframes slide-up {
    from { opacity: 0; transform: translateY(16px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @media (max-width: 640px) {
    .hud {
      padding-bottom: 16px;
    }
  }
</style>
