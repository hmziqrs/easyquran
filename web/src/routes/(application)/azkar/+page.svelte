<script lang="ts">
  import { Seo } from "#lib/components/index.js";
  import { resumeCtxFor, snippetPathFor } from "#lib/data/quran.js";
  import {
    SNIPPET_INDEX_DESCRIPTION,
    SNIPPET_INDEX_TITLE,
    SNIPPET_KIND_LABELS,
    type SnippetText,
  } from "#lib/data/snippets.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { localizeHref } from "#lib/paraglide/runtime.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import ReaderShell from "../_reader/ReaderShell.svelte";

  let { data } = $props();

  const copy = getReaderUiCopy();
  // Prerendered HTML is Arabic-only; after hydration the links honor the
  // reader's active translation, like every index page (§11.6 twin).
  const ctx = $derived(resumeCtxFor(reader.lastRead, { kind: "arabic" }));

  function localized(text: SnippetText): string {
    if (copy.locale === "ar") return text.ar;
    return text.en;
  }

  function detailHref(slug: string): string {
    // Snippet detail paths are public editorial routes, not reader hrefs —
    // localize them directly; localizeHref keeps root-absolute inputs absolute.
    const localized = localizeHref(snippetPathFor(ctx, slug));
    return publicHref(`/${localized.slice(1)}`);
  }

  const title = $derived(localized(SNIPPET_INDEX_TITLE));
  const description = $derived(localized(SNIPPET_INDEX_DESCRIPTION));
</script>

<Seo path="/azkar" title={title} description={description} includeTextVariants={false} />

<ReaderShell textTools={false}>
  {#snippet header()}
    <h1 class="sr-only">{title}</h1>
  {/snippet}

  <div class="mx-auto flex max-w-3xl flex-col gap-8 py-8">
    <header class="flex flex-col gap-2">
      <h2 class="text-xl font-semibold text-foreground">{title}</h2>
      <p class="text-[14px] leading-relaxed text-foreground-secondary">{description}</p>
    </header>

    {#each data.groups as group (group.kind)}
      {#if group.snippets.length > 0}
        <section class="flex flex-col gap-3" aria-labelledby="azkar-kind-{group.kind}">
          <h3
            id="azkar-kind-{group.kind}"
            class="border-b border-reader-divider pb-2 text-[15px] font-medium text-foreground"
          >
            {localized(SNIPPET_KIND_LABELS[group.kind])}
          </h3>
          <ul class="flex flex-col gap-2">
            {#each group.snippets as snippet (snippet.slug)}
              <li>
                <a
                  href={detailHref(snippet.slug)}
                  class="flex flex-col gap-0.5 rounded-lg px-3 py-2.5 transition-colors hover:bg-background-subtle focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
                >
                  <span
                    class="text-[14px] font-medium text-foreground underline-offset-4 hover:underline"
                    >{localized(snippet.title)}</span
                  >
                  <span class="text-[12.5px] leading-relaxed text-foreground-secondary"
                    >{localized(snippet.description)}</span
                  >
                </a>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {/each}
  </div>
</ReaderShell>
