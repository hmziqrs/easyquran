<script lang="ts">
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { page } from "$app/state";
  import { cn } from "$lib/utils";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { online } from "$lib/offline/online.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { Icon } from "$lib/components/icon";
  import { Brand } from "$lib/components/brand";
  import { SearchTrigger } from "$lib/components/search";
  import { stickyNav } from "$lib/stores/sticky-nav.svelte";

  const FOCUSABLE = 'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])';

  let { collapsible = false }: { collapsible?: boolean } = $props();

  let open = $state(false);
  let toggleBtn: HTMLButtonElement | undefined = $state();
  let panelEl: HTMLDivElement | undefined = $state();

  let lastY = 0;
  let ticking = false;

  onMount(() => {
    authState.hydrate();
  });

  function onScroll() {
    if (!collapsible || ticking) return;
    ticking = true;
    requestAnimationFrame(() => {
      const y = window.scrollY;
      if (y <= 8) {
        stickyNav.expand();
      } else if (open) {
        // keep visible while the side panel is open
      } else if (y > lastY + 5) {
        stickyNav.collapse();
      } else if (y < lastY - 5) {
        stickyNav.expand();
      }
      lastY = y;
      ticking = false;
    });
  }

  $effect(() => {
    void page.url.pathname;
    open = false;
    stickyNav.expand();
    lastY = 0;
  });

  let hidden = $derived(collapsible && stickyNav.collapsed && !open);
  let navTop = $derived(hidden ? `-${stickyNav.height}px` : "0px");

  let wasOpen = false;
  $effect(() => {
    const isOpen = open;
    if (isOpen) {
      const first = panelEl?.querySelector<HTMLElement>(FOCUSABLE);
      first?.focus();
    } else if (wasOpen) {
      toggleBtn?.focus();
    }
    wasOpen = isOpen;
  });

  let accountHref = $derived(authState.authenticated ? "/account" : "/login");
  let accountLabel = $derived(authState.authenticated ? "Account" : "Sign in");

  function toggle() {
    open = !open;
  }
  function close() {
    open = false;
  }
  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }
  function onPanelKeydown(e: KeyboardEvent) {
    if (e.key !== "Tab" || !panelEl) return;
    const focusables = Array.from(panelEl.querySelectorAll<HTMLElement>(FOCUSABLE));
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

<svelte:window onkeydown={onKeydown} onscroll={onScroll} />

<nav
  aria-label="Primary"
  style:top={navTop}
  class={cn(
    "sticky z-50 border-b border-line bg-bg/86 backdrop-blur-xl backdrop-saturate-150",
    collapsible && "transition-[top] duration-200 ease-out",
  )}
>
  <div class="flex h-[60px] items-center gap-4 px-5 sm:px-7 lg:px-10">
    <span class="mr-auto" inert={open || undefined} aria-hidden={open || undefined}>
      <Brand />
    </span>

    <div class="flex items-center gap-2">
      {#if online.hydrated && !online.online}
        <span
          class="inline-flex items-center gap-1.5 rounded-full border border-line-2 bg-bg-2 px-2.5 py-1 text-xs text-fg-2"
          role="status"
          aria-label="Offline"
          title="You are offline"
        >
          <span class="sr-only">Offline</span>
          <span class="inline-block size-1.5 rounded-full bg-pop" aria-hidden="true"></span>
          <span class="hidden sm:inline">Offline</span>
        </span>
      {/if}
      <SearchTrigger label="Search the Qur'an" inert={open} />
      <a
        href={accountHref}
        aria-label={accountLabel}
        title={accountLabel}
        inert={open || undefined}
        aria-hidden={open || undefined}
        class="inline-flex h-[38px] w-[38px] items-center justify-center rounded-[11px] border border-line-2 text-fg-2 transition-colors duration-150 hover:bg-bg-2 hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
      >
        <Icon name="user" size={18} title={accountLabel} />
      </a>
      <button
        type="button"
        onclick={toggle}
        aria-expanded={open}
        aria-controls="site-panel"
        aria-label={open ? "Close panel" : "Open panel"}
        title={open ? "Close panel" : "Open panel"}
        bind:this={toggleBtn}
        class="inline-flex h-[38px] w-[38px] items-center justify-center rounded-[11px] border border-line-2 text-fg-2 transition-colors duration-150 hover:bg-bg-2 hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
      >
        <Icon name={open ? "x" : "menu"} size={20} />
      </button>
    </div>
  </div>
</nav>

{#if open}
  <button
    type="button"
    aria-label="Close panel"
    tabindex="-1"
    class="fixed inset-0 top-[60px] z-40 cursor-default bg-bg/40 backdrop-blur-sm"
    transition:fade={{ duration: 150 }}
    onclick={close}
  ></button>
  <div
    id="site-panel"
    role="dialog"
    aria-modal="true"
    aria-label="Site panel"
    tabindex="-1"
    bind:this={panelEl}
    onkeydown={onPanelKeydown}
    transition:fly={{ x: 360, duration: 220, easing: cubicOut }}
    class="fixed bottom-0 right-0 top-[60px] z-50 flex w-[340px] max-w-[88vw] flex-col border-l border-line bg-bg/95 backdrop-blur-xl"
  >
    <div class="flex flex-col gap-6 p-5 sm:p-6">
      <section class="flex flex-col gap-3">
        <h2 class="eyebrow mb-0">Appearance</h2>
        <button
          type="button"
          onclick={() => prefs.toggleTheme()}
          aria-label="Toggle theme"
          class="flex w-full items-center justify-between rounded-lg border border-line-2 bg-bg-1 px-3.5 py-3 text-sm text-fg-2 transition-colors hover:bg-bg-2 hover:text-fg"
        >
          <span class="inline-flex items-center gap-2.5">
            <Icon name={prefs.theme === "dark" ? "moon" : "sun"} size={16} />
            Theme
          </span>
          <span class="text-xs capitalize text-fg-3">{prefs.theme}</span>
        </button>
      </section>

      {#if online.hydrated && !online.online}
        <p
          class="flex items-center gap-2 rounded-lg border border-line-2 bg-bg-1 px-3.5 py-3 text-xs text-fg-3"
          role="status"
        >
          <span class="inline-block size-1.5 rounded-full bg-pop" aria-hidden="true"></span>
          You are offline — cached content only.
        </p>
      {/if}
    </div>
  </div>
{/if}
