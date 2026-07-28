<!--
  Footer — brand blurb + site/links columns, derived from the central site
  config so it never drifts from the nav.
-->
<script lang="ts">
  import { NAV_PAGES, SITE } from "$lib/config/site";
  import { Brand } from "$lib/components/brand";
  import { StatusDot } from "$lib/components/status";
  import { externalLinkAttrs } from "$lib/utils";

  const year = new Date().getFullYear();
  const colHeading = "eyebrow mb-3";
  const link = "text-sm text-fg-2 transition-colors hover:text-fg";
</script>

<footer class="mt-24 border-t border-line bg-bg-elev pb-8 pt-16">
  <div class="mx-auto max-w-[1440px] px-6">
    <div class="grid grid-cols-2 gap-8 pb-12 md:grid-cols-[1.4fr_1fr_1fr]">
      <div>
        <Brand />
        <p class="mt-4 max-w-[36ch] text-sm text-fg-3">
          {SITE.tagline}
        </p>
      </div>

      <div>
        <h2 class={colHeading}>Site</h2>
        <ul class="grid gap-2">
          {#each NAV_PAGES as p (p.id)}
            <li><a class={link} href={p.href}>{p.label}</a></li>
          {/each}
        </ul>
      </div>

      <div>
        <h2 class={colHeading}>Elsewhere</h2>
        <ul class="grid gap-2">
          <li>
            <a class={link} href={SITE.github} {...externalLinkAttrs(SITE.github, { me: true })}
              >GitHub</a
            >
          </li>
          <li><a class={link} href="/llms.txt">llms.txt</a></li>
        </ul>
      </div>
    </div>

    <div class="flex justify-between gap-4 border-t border-line pt-4 font-mono text-xs text-fg-4">
      <span class="flex items-center gap-1.5"><StatusDot /> Free · no ads</span>
      <span>© {year} {SITE.domain}</span>
    </div>
  </div>
</footer>
