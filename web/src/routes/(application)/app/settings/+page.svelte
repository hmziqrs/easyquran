<script lang="ts">
  import { getSettingsCopy } from "$lib/i18n/settings-copy";
  import AppearanceSection from "./_components/AppearanceSection.svelte";
  import StorageSection from "./_components/StorageSection.svelte";
  import ReadingSection from "./_components/ReadingSection.svelte";
  import PrivacySection from "./_components/PrivacySection.svelte";
  import AccountSection from "./_components/AccountSection.svelte";

  const copy = getSettingsCopy();

  const sections = [
    { id: "storage", label: copy.nav.storage },
    { id: "appearance", label: copy.nav.appearance },
    { id: "reading", label: copy.nav.reading },
    { id: "privacy", label: copy.nav.privacy },
    { id: "account", label: copy.nav.account },
  ] as const;

  const pill =
    "rounded-full border border-line-2 bg-bg-1 px-3.5 py-1.5 text-xs text-fg-2 transition-colors hover:border-line hover:text-fg";
</script>

<svelte:head>
  <title>{copy.title} · EasyQuran</title>
</svelte:head>

<div lang={copy.locale} dir={copy.direction} class="mx-auto w-full max-w-2xl px-4 py-8 sm:px-6">
  <h1 class="text-lg font-semibold text-fg">{copy.title}</h1>

  <nav aria-label={copy.sectionsLabel} class="sticky top-[72px] z-30 -mx-1 bg-bg/90 px-1 py-2 backdrop-blur">
    <ul class="flex flex-wrap gap-1.5">
      {#each sections as section (section.id)}
        <li>
          <a href="#{section.id}" class={pill}>{section.label}</a>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="mt-4 grid gap-4">
    <StorageSection id="storage" heading={copy.nav.storage} copy={copy.storage} />
    <AppearanceSection id="appearance" heading={copy.nav.appearance} copy={copy.appearance} />
    <ReadingSection id="reading" heading={copy.nav.reading} copy={copy.reading} />
    <PrivacySection id="privacy" heading={copy.nav.privacy} copy={copy.privacy} />
    <AccountSection id="account" heading={copy.nav.account} copy={copy.account} />
  </div>
</div>
