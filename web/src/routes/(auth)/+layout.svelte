<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { Brand } from "#lib/components/brand/index.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { authFooterLinkIds, authPageTitle, type AuthFooterLinkId } from "#lib/auth/auth-page-meta.js";
  import AuthTopBar from "#lib/auth/components/AuthTopBar.svelte";
  import { installPurgeHook } from "#lib/auth/purge-hook.js";
  import { guestOnlyRedirect } from "#lib/auth/route-guard.js";
  import { getAuthCopy } from "#lib/i18n/auth-copy.js";
  import {
    footer_contact,
    footer_faq,
    footer_privacy,
    footer_read_quran,
    footer_terms,
  } from "#lib/i18n/m/chrome.js";
  import { marketingHref, type MarketingPageId } from "#lib/i18n/marketing.js";
  import { marketingHomeHref } from "#lib/i18n/marketing-copy.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import { publicHref } from "#lib/i18n/public-href.js";

  let { children } = $props();
  let ready = $state(false);

  const copy = getAuthCopy();
  const readQuranLabel = footer_read_quran(undefined, { locale: copy.locale });

  const FOOTER_LINK_META = {
    privacy: { page: "privacy", label: (locale: UiLocale) => footer_privacy(undefined, { locale }) },
    terms: { page: "terms", label: (locale: UiLocale) => footer_terms(undefined, { locale }) },
    contact: {
      page: "contact",
      label: (locale: UiLocale) => footer_contact(undefined, { locale }),
    },
    faq: { page: "faq", label: (locale: UiLocale) => footer_faq(undefined, { locale }) },
  } satisfies Record<
    AuthFooterLinkId,
    { readonly page: MarketingPageId; readonly label: (locale: UiLocale) => string }
  >;

  interface AuthFooterLink {
    readonly href: string;
    readonly label: string;
  }

  const footerLinks = $derived(
    authFooterLinkIds(page.url.pathname)
      .map((id): AuthFooterLink | null => {
        const meta = FOOTER_LINK_META[id];
        const href = marketingHref(meta.page, copy.locale);
        if (href === null) return null;
        return { href: publicHref(href), label: meta.label(copy.locale) };
      })
      .filter((link): link is AuthFooterLink => link !== null),
  );

  // Title lives here, not in each +page.svelte: children are gated behind `ready`,
  // so a page-level title would be missing from the prerendered HTML.
  const pageTitle = $derived(authPageTitle(page.url.pathname));

  async function hydrateRouteAuth(): Promise<void> {
    installPurgeHook(authState);
    authState.hydrate({ force: true });
    await authState.probe();
    const target = guestOnlyRedirect(page.url.pathname, authState.status, authState.user);
    if (target) {
      await goto(publicHref(target), { replace: true });
      ready = true;
      return;
    }
    ready = true;
  }

  onMount(() => {
    void hydrateRouteAuth();
  });
</script>

<svelte:head>
  {#if pageTitle}<title>{pageTitle}</title>{/if}
</svelte:head>

<div
  class="flex min-h-dvh flex-col bg-background lg:flex-row"
  lang={copy.locale}
  dir={copy.direction}
>
  <!-- Auth chrome is one slim row on mobile/tablet; the aside carries the identity on
       desktop. Never a stacked identity block above the form. -->
  <AuthTopBar class="lg:hidden" locale={copy.locale} readLabel={readQuranLabel} />

  <!-- Brand panel: the accent half carries the identity on auth pages so the form
       column stays chrome-free. Hidden below lg — the top bar above takes over. -->
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

  <div class="flex min-w-0 flex-1 items-center justify-center px-5 py-10 sm:px-10 lg:py-14">
    <!-- `main#main` renders unconditionally: the root layout's skip link targets #main, and
         gating it behind `ready` left prerendered auth pages without the anchor, failing the
         build with `no element with id="main" exists`. `tabindex="-1"` makes the skip-link
         focus move land reliably. -->
    <main id="main" tabindex="-1" class="w-full max-w-[420px]">
      {#if ready}
        {@render children()}
      {:else}
        <p class="text-center text-sm text-fg-2" role="status" aria-live="polite">
          Checking your session…
        </p>
      {/if}
      {#if footerLinks.length > 0}
        <!-- Purpose-scoped exits, outside the ready gate so they exist in the prerendered
             HTML too: legal at account creation, support at recovery. -->
        <div
          class="mt-8 flex flex-wrap items-center justify-center gap-x-4 gap-y-1 text-sm text-fg-2"
        >
          {#each footerLinks as link (link.href)}
            <a
              href={link.href}
              class="inline-flex min-h-11 items-center transition-colors hover:text-foreground"
              >{link.label}</a
            >
          {/each}
        </div>
      {/if}
    </main>
  </div>
</div>
