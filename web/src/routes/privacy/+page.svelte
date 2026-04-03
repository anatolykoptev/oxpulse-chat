<script lang="ts">
  import { locale } from '$lib/i18n';
  import LegalLayout from '$lib/LegalLayout.svelte';
  import PrivacyRu from '$lib/legal/PrivacyRu.svelte';
  import PrivacyEn from '$lib/legal/PrivacyEn.svelte';

  let title = $derived($locale === 'ru'
    ? 'Политика конфиденциальности'
    : 'Privacy Policy');
</script>

<LegalLayout {title}>
  {#if $locale === 'ru'}
    <PrivacyRu />
  {:else}
    <PrivacyEn />
  {/if}
</LegalLayout>
