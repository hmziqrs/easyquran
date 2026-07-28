<script lang="ts">
  import { Eyebrow, Section, Seo } from "$lib/components";
  import { SITE } from "$lib/config/site";
</script>

<Seo path="/privacy" />

<Section border={false} width="narrow">
  <Eyebrow>Privacy</Eyebrow>
  <h1 class="mt-4 text-xl">Privacy</h1>

  <div class="mt-6 grid gap-6 text-fg-2">
    <p>
      Placeholder copy — replace this with the real policy before launch. It should describe, at
      minimum, the points below, and it must match what the app actually does.
    </p>

    <div>
      <h2 class="text-lg">What's stored on your device</h2>
      <p class="mt-2 text-sm text-fg-3">
        Reading position, bookmarks and appearance preferences are kept in your browser's local
        storage. They are not sent anywhere.
      </p>
    </div>

    <div>
      <h2 class="text-lg">Analytics</h2>
      <p class="mt-2 text-sm text-fg-3">
        {SITE.name} loads Google Analytics via Firebase to count page views and basic usage. This
        sets cookies and collects an approximate location and device details. Keep this section
        accurate — if analytics is removed, say so; if consent is required in your users'
        jurisdictions, gate it.
      </p>
    </div>

    <div>
      <h2 class="text-lg">No ads</h2>
      <p class="mt-2 text-sm text-fg-3">
        There is no advertising and no ad-network tracking anywhere in {SITE.name}.
      </p>
    </div>
  </div>
</Section>
