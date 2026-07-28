<!--
  App shell — the chrome for the product UI at /app. Deliberately lighter than
  the marketing nav: brand, section tabs, theme toggle. No marketing footer,
  no appearance panel (settings has its own).

  Renders its own <main id="main"> so the root layout's skip link works here
  too. Every /app page passes `noindex` to <Seo>.
-->
<script lang="ts">
  import { page } from "$app/state";
  import { APP_PAGES } from "$lib/config/site";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { cn } from "$lib/utils";
  import { Brand } from "$lib/components/brand";
  import { Icon } from "$lib/components/icon";

  let { children } = $props();

  const tabs = APP_PAGES.filter((p) => p.nav);
  const isActive = (href: string) => page.url.pathname.startsWith(href);
</script>

<div class="flex min-h-screen flex-col">
  <header
    class="sticky top-0 z-50 border-b border-line bg-bg/86 backdrop-blur-xl backdrop-saturate-150"
  >
    <div class="mx-auto flex h-[60px] max-w-[1200px] items-center gap-4 px-4 sm:px-6">
      <Brand href="/app" />

      <!-- section tabs: inline from sm up, bottom bar on small screens -->
      <nav aria-label="App sections" class="ml-2 hidden items-center gap-0.5 sm:flex">
        {#each tabs as t (t.id)}
          <a
            href={t.href}
            aria-current={isActive(t.href) ? "page" : undefined}
            class={cn(
              "rounded px-3 py-1.5 text-sm transition-colors duration-150",
              isActive(t.href) ? "bg-bg-2 text-fg" : "text-fg-3 hover:bg-bg-2 hover:text-fg",
            )}>{t.label}</a
          >
        {/each}
      </nav>

      <button
        type="button"
        onclick={() => prefs.toggleTheme()}
        aria-label="Toggle theme"
        class="ml-auto inline-flex h-11 w-11 items-center justify-center rounded-md text-fg-3 transition-colors duration-150 hover:text-fg"
      >
        <Icon name={prefs.theme === "dark" ? "sun" : "moon"} size={16} />
      </button>
    </div>
  </header>

  <main id="main" tabindex="-1" class="flex-1 pb-20 sm:pb-0">{@render children()}</main>

  <!-- mobile tab bar -->
  <nav
    aria-label="App sections"
    class="fixed inset-x-0 bottom-0 z-50 border-t border-line bg-bg/95 backdrop-blur-xl sm:hidden"
  >
    <div class="mx-auto grid max-w-[1200px] grid-cols-3">
      {#each tabs as t (t.id)}
        <a
          href={t.href}
          aria-current={isActive(t.href) ? "page" : undefined}
          class={cn(
            "flex flex-col items-center gap-1 py-2.5 text-xs transition-colors",
            isActive(t.href) ? "text-accent" : "text-fg-3 hover:text-fg",
          )}
        >
          <Icon
            name={t.id === "read" ? "book" : t.id === "bookmarks" ? "bookmark" : "gear"}
            size={18}
          />
          {t.label}
        </a>
      {/each}
    </div>
  </nav>
</div>
