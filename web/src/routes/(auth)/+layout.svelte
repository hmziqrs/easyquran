<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { Brand } from "$lib/components/brand";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { installPurgeHook } from "$lib/auth/purge-hook";
  import { guestOnlyRedirect } from "$lib/auth/route-guard";
  import { getAuthCopy } from "$lib/i18n/auth-copy";
  import { marketingHomeHref } from "$lib/i18n/marketing-copy";

  let { children } = $props();
  let ready = $state(false);

  const copy = getAuthCopy();

  async function hydrateRouteAuth(): Promise<void> {
    installPurgeHook(authState);
    authState.hydrate({ force: true });
    await authState.probe();
    const target = guestOnlyRedirect(page.url.pathname, authState.status, authState.user);
    if (target) {
      await goto(resolve(target), { replaceState: true });
      ready = true;
      return;
    }
    ready = true;
  }

  onMount(() => {
    void hydrateRouteAuth();
  });
</script>

<div class="flex min-h-dvh bg-background" lang={copy.locale} dir={copy.direction}>
  <!-- Brand panel: the accent half carries the identity on auth pages so the form
       column stays chrome-free. Hidden below lg — mobile gets the bare form. -->
  <aside
    class="relative hidden overflow-hidden bg-primary p-10 text-primary-foreground lg:flex lg:w-[45%] lg:flex-col lg:justify-between xl:p-16"
  >
    <div class="relative">
      <Brand tone="on-accent" homeHref={marketingHomeHref(copy.locale)} />
    </div>
    <figure class="relative flex flex-col gap-5">
      <blockquote
        class="text-balance font-arabic text-[clamp(26px,2.6vw,38px)] leading-[2] text-primary-foreground"
        lang="ar"
        dir="rtl"
      >
        أَلَا بِذِكْرِ اللَّهِ تَطْمَئِنُّ الْقُلُوبُ
      </blockquote>
      <figcaption class="flex items-center gap-3 text-sm text-primary-foreground/70">
        <span aria-hidden="true" class="h-px w-8 bg-primary-foreground/40"></span>
        <span lang="ar" dir="rtl">سورة الرعد · ١٣:٢٨</span>
      </figcaption>
    </figure>
    <p class="relative max-w-[38ch] text-sm leading-relaxed text-primary-foreground/70">
      {copy.dialogDescription}
    </p>
  </aside>

  <div class="flex min-w-0 flex-1 items-center justify-center px-5 py-14 sm:px-10">
    <!-- `main#main` renders unconditionally: the root layout's skip link targets #main, and
         gating it behind `ready` left prerendered auth pages without the anchor, failing the
         build with `no element with id="main" exists`. -->
    <main id="main" class="w-full max-w-[420px]">
      {#if ready}
        {@render children()}
      {:else}
        <p class="text-center text-sm text-fg-2" role="status" aria-live="polite">
          Checking your session…
        </p>
      {/if}
    </main>
  </div>
</div>
