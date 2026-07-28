<script lang="ts">
  import { Button, Card, Eyebrow, Section, Seo } from "$lib/components";
  import { PAGE_META } from "$lib/config/site";
</script>

<Seo path="/download" />

<Section border={false} width="narrow">
  <Eyebrow>Download</Eyebrow>
  <h1 class="mt-4 text-xl">Get EasyQuran</h1>
  <p class="mt-4 text-fg-2">{PAGE_META.download.description}</p>

  <div class="mt-10 grid gap-4 sm:grid-cols-2">
    <Card>
      <h2 class="text-lg">In the browser</h2>
      <p class="mt-2 text-sm text-fg-3">
        Nothing to install — it works offline once loaded and installs as an app from your
        browser's menu.
      </p>
      <div class="mt-5">
        <Button variant="accent" href="/app" arrow>Open EasyQuran</Button>
      </div>
    </Card>

    <Card>
      <h2 class="text-lg">Native apps</h2>
      <p class="mt-2 text-sm text-fg-3">
        Placeholder — link the App Store / Play Store builds here when they ship.
      </p>
      <div class="mt-5">
        <Button variant="ghost" disabled>Coming soon</Button>
      </div>
    </Card>
  </div>
</Section>
