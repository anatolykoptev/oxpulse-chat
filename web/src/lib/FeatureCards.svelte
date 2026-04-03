<script lang="ts">
  import type { Translations } from './i18n';

  let { t }: { t: Translations } = $props();
</script>

<ul class="features">
  <li class="feature">
    <svg class="feature-icon" width="16" height="16" viewBox="0 0 24 24" fill="none">
      <path d="M12 2L3 7v5c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V7l-9-5z" fill="var(--accent)" opacity="0.2"/>
      <path d="M9 12l2 2 4-4" stroke="var(--accent)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
    <span>{t.featureE2ee}</span>
  </li>
  <li class="feature">
    <svg class="feature-icon" width="16" height="16" viewBox="0 0 24 24" fill="none">
      <rect x="3" y="3" width="18" height="18" rx="5" fill="var(--accent)" opacity="0.2"/>
      <path d="M8 12h8M12 8v8" stroke="var(--accent)" stroke-width="2" stroke-linecap="round"/>
    </svg>
    <span>{t.featureNoSignup}</span>
  </li>
  <li class="feature">
    <svg class="feature-icon" width="16" height="16" viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="9" fill="var(--accent)" opacity="0.2"/>
      <path d="M8.5 12.5a5 5 0 017 0M10 14a2.5 2.5 0 014 0" stroke="var(--accent)" stroke-width="2" stroke-linecap="round" transform="rotate(180 12 12)"/>
      <circle cx="12" cy="12" r="1.2" fill="var(--accent)"/>
    </svg>
    <span>{t.featureRelay}</span>
  </li>
</ul>

<style>
  .features {
    display: flex;
    flex-direction: column;
    gap: clamp(6px, 1dvh, 12px);
    list-style: none;
    margin: 0;
    padding: 0;
    animation: features-in 1s cubic-bezier(0.16, 1, 0.3, 1) both;
    animation-delay: 0.55s;
  }

  @keyframes features-in {
    from { opacity: 0; transform: translateY(16px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .feature {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: clamp(13px, 1.2vw, 16px);
    font-weight: 400;
    color: rgba(255, 255, 255, 0.6);
    letter-spacing: 0.15px;
  }

  .feature-icon {
    flex-shrink: 0;
    width: clamp(16px, 1.5vw, 22px);
    height: clamp(16px, 1.5vw, 22px);
  }
</style>
