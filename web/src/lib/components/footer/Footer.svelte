<!--
  Footer — per the design comp: a 4-column grid (brand blurb · Product ·
  Company · Legal) on an inset ground, with a bottom row carrying the
  copyright on the left and an Arabic line on the right. Links come from the
  central site config so it never drifts from the nav.
-->
<script lang="ts">
  import { NAV_LINKS, SITE } from "$lib/config/site";

  const year = new Date().getFullYear();
  const brand = SITE.name.toLowerCase();

  // Company column = the nav's marketing links (Read lives in Product).
  const companyLinks = NAV_LINKS.filter((p) => p.href !== "/app");
  const productLinks: { href: string; label: string }[] = [
    { href: "/app", label: "Read the Qur'an" },
    { href: "/app", label: "Bookmarks" },
    { href: "/", label: "What's inside" },
  ];
  const legalLinks: { href: string; label: string }[] = [
    { href: "/privacy", label: "Privacy Policy" },
    { href: "/terms", label: "Terms of Service" },
  ];

  const colHeading = "eyebrow mb-0";
  const link = "text-[14.5px] text-fg-2 transition-colors hover:text-fg";
</script>

<footer class="border-t border-line bg-bg-2">
  <div class="mx-auto max-w-[1180px] px-6 sm:px-7">
    <div
      class="grid grid-cols-2 gap-8 pt-[52px] pb-10 md:grid-cols-[1.4fr_1fr_1fr_1fr] md:gap-9"
    >
      <!-- Brand blurb -->
      <div class="col-span-2 flex flex-col gap-3 md:col-span-1">
        <div class="flex items-center gap-2.5">
          <span
            class="flex size-7 items-center justify-center rounded-[9px] bg-accent font-arabic text-[16px] leading-none text-accent-fg"
            aria-hidden="true">ق</span
          >
          <span class="text-[17px] font-semibold tracking-[-0.02em] text-fg">{brand}</span>
        </div>
        <p class="max-w-[30ch] text-[14.5px] leading-relaxed text-fg-2">
          {SITE.footerBlurb}
        </p>
      </div>

      <!-- Product -->
      <nav aria-label="Product" class="flex flex-col gap-2.5">
        <h2 class={colHeading}>Product</h2>
        <ul class="grid gap-2.5">
          {#each productLinks as p (p.label)}
            <li><a class={link} href={p.href}>{p.label}</a></li>
          {/each}
        </ul>
      </nav>

      <!-- Company -->
      <nav aria-label="Company" class="flex flex-col gap-2.5">
        <h2 class={colHeading}>Company</h2>
        <ul class="grid gap-2.5">
          {#each companyLinks as p (p.href)}
            <li><a class={link} href={p.href}>{p.label}</a></li>
          {/each}
        </ul>
      </nav>

      <!-- Legal -->
      <nav aria-label="Legal" class="flex flex-col gap-2.5">
        <h2 class={colHeading}>Legal</h2>
        <ul class="grid gap-2.5">
          {#each legalLinks as p (p.label)}
            <li><a class={link} href={p.href}>{p.label}</a></li>
          {/each}
        </ul>
      </nav>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-5 pb-11">
      <span class="text-[13.5px] text-fg-3">© {year} {brand}. Made with care.</span>
      <span dir="rtl" class="font-arabic text-[17px] leading-none text-fg-3">
        وَنَزَّلْنَا عَلَيْكَ الْكِتَابَ تِبْيَانًا لِّكُلِّ شَيْءٍ
      </span>
    </div>
  </div>
</footer>
