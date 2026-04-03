<script lang="ts">
  import { locale } from '$lib/i18n';
  import LegalLayout from '$lib/LegalLayout.svelte';
  import AccessibilityRu from '$lib/legal/AccessibilityRu.svelte';
  import AccessibilityEn from '$lib/legal/AccessibilityEn.svelte';

  let title = $derived($locale === 'ru'
    ? 'Доступность'
    : 'Accessibility');
</script>

<LegalLayout {title}>
  {#if $locale === 'ru'}
    <AccessibilityRu />
  {:else}
    <AccessibilityEn />
  {/if}
</LegalLayout>
