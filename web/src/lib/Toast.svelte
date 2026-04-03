<script lang="ts">
  let { visible = false, message = '' }: { visible?: boolean; message?: string } = $props();
</script>

<div class="toast" class:show={visible} role="status" aria-live="polite">
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <polyline points="20 6 9 17 4 12"/>
  </svg>
  {message}
</div>

<style>
  .toast {
    position: fixed;
    bottom: 100px;
    left: 50%;
    transform: translateX(-50%) translateY(8px);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 22px;
    background: var(--glass);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border-radius: 999px;
    border: 1px solid rgba(201, 169, 110, 0.25);
    color: var(--accent);
    font-family: var(--font);
    font-size: 13px;
    font-weight: 500;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.3s, transform 0.3s;
    z-index: 30;
  }

  .toast.show {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
</style>
