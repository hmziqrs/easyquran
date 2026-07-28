<script lang="ts">
  import { Button, Section, Seo } from "$lib/components";
  import { SITE } from "$lib/config/site";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { track } from "$lib/firebase";

  function toggleTheme() {
    prefs.toggleTheme();
    void track("theme_toggle", { theme: prefs.theme });
  }
</script>

<Seo path="/" />

<Section border={false} width="narrow">
  <p class="eyebrow">{SITE.domain}</p>
  <h1 class="mt-4 text-2xl">{SITE.name}</h1>
  <p class="mt-4 text-fg-2">{SITE.tagline}</p>

  <p class="arabic mt-10 text-accent">بِسْمِ ٱللَّٰهِ ٱلرَّحْمَٰنِ ٱلرَّحِيمِ</p>

  <div class="mt-10 flex flex-wrap gap-3">
    <Button variant="accent" href="/about" arrow>About</Button>
    <Button variant="ghost" onclick={toggleTheme}>Toggle theme</Button>
  </div>
</Section>
