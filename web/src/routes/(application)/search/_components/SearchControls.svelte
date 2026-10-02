<script lang="ts">
  import type { SearchCopy } from "$lib/i18n/search-copy";

  let {
    copy,
    query,
    searching,
    selected,
    pickerOpen,
    onQuery,
    onRemove,
    onTogglePicker,
  }: {
    copy: SearchCopy;
    query: string;
    searching: boolean;
    selected: { id: string; name: string }[];
    pickerOpen: boolean;
    onQuery: (value: string) => void;
    onRemove: (id: string) => void;
    onTogglePicker: () => void;
  } = $props();
</script>

<div class="flex flex-col gap-3">
  <div>
    <label for="search-query" class="sr-only">{copy.inputLabel}</label>
    <input
      id="search-query"
      type="search"
      value={query}
      oninput={(event) => onQuery(event.currentTarget.value)}
      placeholder={copy.placeholder}
      aria-busy={searching}
      autocomplete="off"
      spellcheck="false"
      class="h-11 w-full rounded-xl border border-border-strong bg-background-subtle px-4 text-[14.5px] text-foreground shadow-none outline-none transition-colors placeholder:text-muted focus:border-border-strong focus:ring-2 focus:ring-focus-ring/40"
    />
  </div>

  <div class="flex flex-wrap items-center gap-1.5">
    <button
      type="button"
      onclick={onTogglePicker}
      aria-expanded={pickerOpen}
      class="flex items-center gap-1.5 rounded-pill border border-border-strong bg-surface px-3.5 py-1.5 text-[13px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground"
    >
      {copy.filterOpen}
      {#if selected.length > 0}
        <span class="text-muted">·</span>
        <span class="text-muted">{copy.filterSelected(selected.length)}</span>
      {/if}
    </button>

    {#each selected as chip (chip.id)}
      <!-- §33 active state: a selected filter chip carries the primary fill + white
           text (tone mirrors Chip.svelte `active`), not the grey outline. -->
      <span
        class="flex items-center gap-1 rounded-pill border border-transparent bg-primary px-3 py-1 text-[12.5px] font-medium text-primary-foreground"
      >
        <span class="max-w-[220px] truncate">{chip.name}</span>
        <button
          type="button"
          onclick={() => onRemove(chip.id)}
          aria-label={`${chip.name} ×`}
          class="p-0.5 opacity-80 transition-opacity hover:opacity-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-foreground"
        >
          ×
        </button>
      </span>
    {/each}
  </div>
</div>
