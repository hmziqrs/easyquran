<script lang="ts">
  import { Button, Container, Eyebrow, Seo } from "$lib/components";
</script>

<Seo path="/app/bookmarks" noindex />

<Container width="narrow" class="py-10">
  <Eyebrow>Bookmarks</Eyebrow>
  <h1 class="mt-3 text-xl">Saved ayahs</h1>

  <!-- empty state — replace once bookmarks are persisted -->
  <div
    class="mt-8 flex flex-col items-center gap-3 rounded-lg border border-dashed border-line-2 px-6 py-16 text-center"
  >
    <p class="text-sm text-fg-2">Nothing saved yet.</p>
    <p class="max-w-[38ch] text-sm text-fg-3">
      Bookmarks will be stored on this device, alongside your reading position.
    </p>
    <div class="mt-2">
      <Button variant="ghost" size="sm" href="/app/read" arrow>Start reading</Button>
    </div>
  </div>
</Container>
