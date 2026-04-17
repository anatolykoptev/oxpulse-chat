<script lang="ts">
  import { branding } from '$lib/branding';

  interface Props { variant?: 'light' | 'dark'; class?: string; }
  let { variant = 'light', class: klass = '' }: Props = $props();
  const src = $derived(variant === 'dark' ? $branding.logo_dark : $branding.logo_light);
  const alt = $derived($branding.site_name);
</script>

<img {src} {alt} class={klass} />
