<script lang="ts">
  import { ICONS, type IconName } from "./icons";

  let {
    name,
    size = 16,
    class: className = "",
    title,
  }: { name: IconName; size?: number | string; class?: string; title?: string } = $props();

  const icon = $derived(ICONS[name]);
  const labelled = $derived(title !== undefined);
</script>

{#if icon}
  <svg
    class={className}
    width={size}
    height={size}
    viewBox={icon.viewBox}
    fill="currentColor"
    role={labelled ? "img" : "presentation"}
    aria-hidden={labelled ? undefined : "true"}
    aria-label={title}
  >
    {#if labelled}<title>{title}</title>{/if}
    <path d={icon.d} />
  </svg>
{/if}
