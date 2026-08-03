<script lang="ts">
  import { page } from "$app/state";
  import { NAV_LINKS } from "$lib/config/site";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { online } from "$lib/offline/online.svelte";
  import { cn } from "$lib/utils";
  import { Button } from "$lib/components/ui/button";
  import { Brand } from "$lib/components/brand";
  import { Icon } from "$lib/components/icon";

  let open = $state(false);
  let toggleBtn: HTMLButtonElement | undefined = $state();
  let drawerEl: HTMLDivElement | undefined = $state();

  function isActive(href: string) {
    return href === "/app" ? page.url.pathname.startsWith("/app") : page.url.pathname === href;
  }

  $effect(() => {
    void page.url.pathname;
    open = false;
  });

  let wasOpen = false;
  $effect(() => {
    const isOpen = open;
    if (isOpen) {
      const first = drawerEl?.querySelector<HTMLElement>(
        'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      first?.focus();
    } else if (wasOpen) {
      toggleBtn?.focus();
    }
    wasOpen = isOpen;
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
  function onDrawerKeydown(e: KeyboardEvent) {
    if (e.key !== "Tab" || !drawerEl) return;
    const focusables = Array.from(
      drawerEl.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    );
    if (focusables.length === 0) return;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<nav
  aria-label="Primary"
  class="sticky top-0 z-50 border-b border-line bg-bg/86 backdrop-blur-xl backdrop-saturate-150"
>
  <div class="mx-auto flex h-[60px] max-w-[1180px] items-center gap-6 px-6 sm:px-7">
    <Brand class="mr-auto" />

    <div class="hidden items-center gap-0.5 md:flex">
      {#each NAV_LINKS as p (p.href)}
        {@const active = isActive(p.href)}
        <a
          href={p.href}
          aria-current={active ? "page" : undefined}
          class={cn(
            "rounded-md px-3 py-1.5 text-sm transition-colors duration-150",
            active ? "bg-bg-2 text-fg" : "text-fg-2 hover:bg-bg-2 hover:text-fg",
          )}>{p.label}</a
        >
      {/each}
    </div>

    <div class="flex items-center gap-2">
      {#if online.hydrated && !online.online}
        <span
          class="inline-flex items-center gap-1.5 rounded-full border border-line-2 bg-bg-2 px-2.5 py-1 text-xs text-fg-2"
          role="status"
          aria-label="Offline"
          title="You are offline"
        >
          <span class="inline-block size-1.5 rounded-full bg-pop" aria-hidden="true"></span>
          <span class="hidden sm:inline">Offline</span>
        </span>
      {/if}
      <button
        type="button"
        onclick={() => prefs.toggleTheme()}
        aria-label="Toggle theme"
        class="inline-flex h-[34px] w-[34px] items-center justify-center rounded-[10px] border border-line-2 text-fg-2 transition-colors duration-150 hover:bg-bg-2 hover:text-fg"
      >
        <Icon name={prefs.theme === "dark" ? "sun" : "moon"} size={16} />
      </button>
      <Button variant="accent" size="sm" href="/app" class="hidden sm:inline-flex">
        Open the app
      </Button>
      <button
        type="button"
        onclick={toggle}
        aria-expanded={open}
        aria-controls="mobile-menu"
        aria-label="Menu"
        bind:this={toggleBtn}
        class="inline-flex h-[34px] w-[34px] items-center justify-center rounded-[10px] border border-line-2 text-fg-2 transition-colors duration-150 hover:bg-bg-2 hover:text-fg md:hidden"
      >
        <Icon name={open ? "x" : "menu"} size={20} />
      </button>
    </div>
  </div>
</nav>

{#if open}
  <button
    type="button"
    aria-label="Close menu"
    tabindex="-1"
    class="fixed inset-0 top-[60px] z-40 cursor-default bg-bg/40 backdrop-blur-sm md:hidden"
    onclick={close}
  ></button>
  <div
    id="mobile-menu"
    role="dialog"
    aria-modal="true"
    aria-label="Mobile"
    tabindex="-1"
    bind:this={drawerEl}
    onkeydown={onDrawerKeydown}
    class="fixed inset-x-0 top-[60px] z-50 border-b border-line bg-bg/95 backdrop-blur-xl md:hidden"
  >
    <div class="mx-auto max-w-[1180px] px-6 py-3 sm:px-7">
      <div class="grid gap-1">
        {#each NAV_LINKS as p (p.href)}
          {@const active = isActive(p.href)}
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
      <div class="mt-2 border-t border-line pt-3">
        <Button variant="accent" size="sm" href="/app" class="w-full justify-center">
          Open the app
        </Button>
      </div>
    </div>
  </div>
{/if}
