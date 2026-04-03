<script lang="ts">
  import { locale } from '$lib/i18n';
  import LegalLayout from '$lib/LegalLayout.svelte';
  import TermsRu from '$lib/legal/TermsRu.svelte';
  import TermsEn from '$lib/legal/TermsEn.svelte';

  let title = $derived($locale === 'ru'
    ? 'Условия использования'
    : 'Terms of Service');
</script>

<LegalLayout {title}>
  {#if $locale === 'ru'}
    <TermsRu />
  {:else}
    <TermsEn />
  {/if}
</LegalLayout>
