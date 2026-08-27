<script lang="ts">
  import { appLocale, setAppLocale } from "$lib/i18n/app-locale";
  import { getBookmarksCopy } from "$lib/i18n/bookmarks-copy";
  import type { UiLocale } from "$lib/i18n/locales";

  // Mirrors the app layout seam: publish first, then resolve page copy from the
  // hand-off — the same order +page.svelte sees (layout init runs before the page's).
  let { locale = null }: { locale?: UiLocale | null } = $props();
  // svelte-ignore state_referenced_locally -- the init-only read is the point: the real layout also
  // captures its locale exactly once at init, and the harness never re-publishes on prop updates.
  if (locale !== null) setAppLocale(locale);
  const copy = getBookmarksCopy(appLocale());
</script>

<p data-locale={copy.locale} data-direction={copy.direction}>{copy.title} · {copy.unfiled} · {copy.foldersHeading}</p>
