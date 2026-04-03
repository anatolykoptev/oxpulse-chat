<script lang="ts">
  let { intensity = 0 }: { intensity?: number } = $props();
</script>

<div class="pulse-anchor">
  <div class="signal-pulse" style="--intensity: {intensity}">
    <div class="pulse-wave wave-1"></div>
    <div class="pulse-wave wave-2"></div>
    <div class="pulse-wave wave-3"></div>
    <div class="pulse-wave wave-4"></div>
    <div class="pulse-core-glow"></div>
  </div>
</div>

<style>
  .pulse-anchor {
    position: fixed;
    top: 38%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 1;
    pointer-events: none;
  }

  .signal-pulse {
    position: relative;
    width: 280px;
    height: 280px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .pulse-wave {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1px solid var(--accent);
    opacity: 0;
    animation: pulse-expand 4s ease-out infinite;
  }

  .wave-2 { animation-delay: 1s; }
  .wave-3 { animation-delay: 2s; }
  .wave-4 { animation-delay: 3s; }

  @keyframes pulse-expand {
    0% { transform: scale(0.15); opacity: calc(0.25 + var(--intensity, 0) * 0.25); }
    100% { transform: scale(1.8); opacity: 0; }
  }

  .pulse-core-glow {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow:
      0 0 20px rgba(201, 169, 110, 0.4),
      0 0 60px rgba(201, 169, 110, 0.15);
    animation: core-glow 3s ease-in-out infinite;
  }

  @keyframes core-glow {
    0%, 100% { transform: scale(1); box-shadow: 0 0 20px rgba(201, 169, 110, 0.4), 0 0 60px rgba(201, 169, 110, 0.15); }
    50% { transform: scale(1.3); box-shadow: 0 0 30px rgba(201, 169, 110, 0.5), 0 0 80px rgba(201, 169, 110, 0.2); }
  }

  @media (max-width: 480px) {
    .pulse-anchor { top: 30%; }
    .signal-pulse { width: 180px; height: 180px; }
  }

  @media (max-height: 680px) {
    .pulse-anchor { top: 25%; }
    .signal-pulse { width: 160px; height: 160px; }
  }
</style>
