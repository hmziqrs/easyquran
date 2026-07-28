<!--
  Nav — sticky, blurred top bar. Brand + section links (from site config) +
  theme toggle and GitHub. The active link mirrors the current route via
  SvelteKit's `page` state. Below `md` the links collapse into a hamburger
  drawer (the bar keeps brand + theme toggle).
-->
<script lang="ts">
  import { page } from "$app/state";
  import { NAV_PAGES, SITE } from "$lib/config/site";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { cn } from "$lib/utils";
  import { Button } from "$lib/components/ui/button";
  import { Brand } from "$lib/components/brand";
  import { Icon } from "$lib/components/icon";

  const links = NAV_PAGES.filter((p) => p.id !== "home");
  let open = $state(false);

  // Close the drawer whenever the route changes (link taps + back/forward).
  $effect(() => {
    void page.url.pathname;
    open = false;
  });

  function toggle() {
    open = !open;
  }
  function close() {
    open = false;
  }
  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }
</script>

<svelte:window onkeydown={onKeydown} />

<nav
  aria-label="Primary"
  class="sticky top-0 z-50 border-b border-line bg-bg/86 backdrop-blur-xl backdrop-saturate-150"
  style="--nav-h:60px"
>
  <div class="mx-auto flex h-[60px] max-w-[1440px] items-center gap-4 px-6 md:gap-8">
    <Brand />

    <div class="ml-4 hidden items-center gap-0.5 md:flex">
      {#each links as p (p.id)}
        {@const active = page.url.pathname === p.href}
        <a
          href={p.href}
          aria-current={active ? "page" : undefined}
          class={cn(
            "rounded px-3 py-1.5 text-sm transition-colors duration-150",
            active ? "bg-bg-2 text-fg" : "text-fg-3 hover:bg-bg-2 hover:text-fg",
          )}>{p.label}</a
        >
      {/each}
    </div>

    <div class="ml-auto flex items-center gap-2">
      <button
        type="button"
        onclick={() => prefs.toggleTheme()}
        aria-label="Toggle theme"
        class="inline-flex h-11 w-11 items-center justify-center rounded-md text-fg-3 transition-colors duration-150 hover:text-fg"
      >
        <Icon name={prefs.theme === "dark" ? "sun" : "moon"} size={16} />
      </button>
      <Button
        variant="ghost"
        size="sm"
        href={SITE.github}
        aria-label="GitHub (opens in a new tab)"
        class="hidden md:inline-flex"
      >
        <Icon name="gh" size={14} /> <span class="hidden lg:inline">GitHub</span>
      </Button>
      <Button variant="accent" size="sm" href="/app" arrow class="hidden sm:inline-flex">
        Open app
      </Button>
      <button
        type="button"
        onclick={toggle}
        aria-expanded={open}
        aria-controls="mobile-menu"
        aria-label="Menu"
        class="inline-flex h-11 w-11 items-center justify-center rounded-md text-fg-3 transition-colors duration-150 hover:bg-bg-2 hover:text-fg md:hidden"
      >
        <Icon name={open ? "x" : "menu"} size={20} />
      </button>
    </div>
  </div>
</nav>

{#if open}
  <!-- click-away backdrop -->
  <button
    type="button"
    aria-label="Close menu"
    tabindex="-1"
    class="fixed inset-0 top-[60px] z-40 cursor-default bg-bg/40 backdrop-blur-sm md:hidden"
    onclick={close}
  ></button>
  <div
    id="mobile-menu"
    class="fixed inset-x-0 top-[60px] z-50 border-b border-line bg-bg/95 backdrop-blur-xl md:hidden"
  >
    <div class="mx-auto max-w-[1440px] px-6 py-3">
      <div class="grid gap-1">
        {#each links as p (p.id)}
          {@const active = page.url.pathname === p.href}
          <a
            href={p.href}
            aria-current={active ? "page" : undefined}
            class={cn(
              "rounded-md px-3 py-2.5 text-sm transition-colors",
              active ? "bg-bg-2 text-fg" : "text-fg-2 hover:bg-bg-2 hover:text-fg",
            )}
            onclick={close}>{p.label}</a
          >
        {/each}
      </div>
      <div class="mt-2 grid gap-2 border-t border-line pt-3 sm:grid-cols-2">
        <Button variant="ghost" size="sm" href={SITE.github} class="w-full justify-center">
          <Icon name="gh" size={14} /> GitHub
        </Button>
        <Button variant="accent" size="sm" href="/app" arrow class="w-full justify-center">
          Open app
        </Button>
      </div>
    </div>
  </div>
{/if}
