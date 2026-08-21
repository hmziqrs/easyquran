<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/utils";
  import type { SettingsCopy } from "$lib/i18n/settings-copy";

  let {
    copy,
    sections,
    active,
    onSelect,
    children,
  }: {
    copy: Pick<SettingsCopy, "title" | "sectionsLabel">;
    sections: { id: string; label: string }[];
    active: string;
    onSelect: (id: string) => void;
    children: Snippet;
  } = $props();

  function navItemClass(isActive: boolean): string {
    const base =
      "flex items-center whitespace-nowrap rounded-full border px-3.5 py-1.5 text-sm transition-colors md:w-full md:rounded-md md:border-0 md:bg-transparent md:px-3 md:text-start";
    if (isActive) {
      return cn(base, "border-accent bg-accent-soft font-medium text-fg md:bg-accent-soft");
    }
    return cn(
      base,
      "border-line-2 bg-bg-1 text-fg-2 hover:border-line hover:text-fg md:hover:bg-bg-2",
    );
  }
</script>

<div class="mx-auto w-full max-w-5xl px-4 py-8 sm:px-6 lg:px-8">
  <h1 class="text-lg font-semibold text-fg">{copy.title}</h1>

  <div class="mt-5 flex flex-col gap-6 md:mt-6 md:flex-row md:gap-0">
    <nav
      aria-label={copy.sectionsLabel}
      class="md:sticky md:top-24 md:w-56 md:shrink-0 md:self-start md:border-e md:pe-6"
    >
      <ul
        class="-mx-1 flex gap-1.5 overflow-x-auto px-1 pb-1 md:mx-0 md:block md:space-y-0.5 md:overflow-visible md:px-0 md:pb-0"
      >
        {#each sections as section (section.id)}
          <li class="shrink-0 md:shrink">
            <a
              href={"#" + section.id}
              aria-current={section.id === active ? "page" : undefined}
              class={navItemClass(section.id === active)}
              onclick={(event) => {
                event.preventDefault();
                onSelect(section.id);
              }}
            >
              {section.label}
            </a>
          </li>
        {/each}
      </ul>
    </nav>

    <div class="min-w-0 flex-1 md:ps-8" aria-live="polite">
      {@render children()}
    </div>
  </div>
</div>
