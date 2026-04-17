<script lang="ts">
  import { branding } from '$lib/branding';

  let locale = $state<'ru' | 'en'>('en');
  $effect(() => {
    locale = ((document.documentElement.lang as 'ru' | 'en') ?? 'en') || 'en';
  });

  const affiliate = $derived($branding.affiliate);
</script>

{#if affiliate}
  <a href={affiliate.vpn_cta_url} target="_blank" rel="noopener" class="banner">
    {locale === 'ru' ? affiliate.vpn_cta_text_ru : affiliate.vpn_cta_text_en}
  </a>
{/if}

<style>
  .banner {
    display: inline-block;
    padding: 10px 16px;
    background: var(--accent, #C9A96E);
    color: #06060A;
    border-radius: 999px;
    text-decoration: none;
    font-weight: 600;
  }
</style>
