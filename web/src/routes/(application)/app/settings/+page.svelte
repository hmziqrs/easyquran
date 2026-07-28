<script lang="ts">
  import { Container, Eyebrow, Seo } from "$lib/components";
  import { ACCENTS, type ThemeMode } from "$lib/config/site";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { cn } from "$lib/utils";

  const themes: ThemeMode[] = ["dark", "light"];

  const pill = "rounded-md border px-3 py-1.5 text-sm capitalize transition-colors duration-150";
  const on = "border-accent bg-accent-soft text-fg";
  const off = "border-line-2 text-fg-2 hover:text-fg";
</script>

<Seo path="/app/settings" noindex />

<Container width="narrow" class="py-10">
  <Eyebrow>Settings</Eyebrow>
  <h1 class="mt-3 text-xl">Appearance</h1>

  <div class="mt-8 grid gap-8">
    <div>
      <h2 class="text-sm text-fg-2">Theme</h2>
      <div class="mt-3 flex gap-2">
        {#each themes as t (t)}
          <button
            type="button"
            class={cn(pill, prefs.theme === t ? on : off)}
            aria-pressed={prefs.theme === t}
            onclick={() => prefs.setTheme(t)}>{t}</button
          >
        {/each}
      </div>
    </div>

    <div>
      <h2 class="text-sm text-fg-2">Accent</h2>
      <div class="mt-3 flex flex-wrap gap-2">
        {#each ACCENTS as a (a.id)}
          <button
            type="button"
            title={a.label}
            aria-label={`Accent: ${a.label}`}
            aria-pressed={prefs.accent === a.id}
            class={cn(
              "size-9 rounded-lg border-2 transition-transform",
              prefs.accent === a.id ? "scale-110 border-fg" : "border-line",
            )}
            style={`background:${a.hex}`}
          ></button>
        {/each}
      </div>
    </div>

    <p class="text-sm text-fg-3">
      Placeholder — reading preferences (script, translation, font size) belong here too. Choices
      persist to this device.
    </p>
  </div>
</Container>
