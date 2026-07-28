<script lang="ts">
  import { Button, Card, Eyebrow, Icon, Seo } from "$lib/components";
  import { Input } from "$lib/components/ui/input";
  import { Textarea } from "$lib/components/ui/textarea";
  import { Label } from "$lib/components/ui/label";
  import { cn } from "$lib/utils";
  import { CONTACT_TOPICS } from "$lib/data/content";
  import { SITE } from "$lib/config/site";

  // Local-only form state (no backend, per spec 8F).
  let name = $state("");
  let email = $state("");
  let topic = $state<string | null>(null);
  let message = $state("");
  let sent = $state(false);

  function send() {
    sent = true;
  }

  function reset() {
    name = "";
    email = "";
    topic = null;
    message = "";
    sent = false;
  }
</script>

<Seo path="/contact" schemaSubtype="ContactPage" />

<div
  class="mx-auto grid w-full max-w-[1000px] gap-14 px-7 pt-[72px] pb-24 md:grid-cols-2 items-start"
>
  <!-- Left: intro + contact details -->
  <div class="flex flex-col gap-6">
    <div class="flex flex-col gap-3">
      <Eyebrow class="text-accent">Contact</Eyebrow>
      <h1 class="text-2xl">Say salam.</h1>
      <p class="text-[18px] leading-[1.65] text-fg-2">
        Bug reports, script corrections, or a feature you keep wishing for — all welcome.
      </p>
    </div>

    <div class="flex flex-col gap-3.5">
      <div class="flex flex-col gap-[3px]">
        <span class="text-xs text-fg-3">Email</span>
        <a
          class="text-base text-fg transition-colors hover:text-accent"
          href="mailto:{SITE.email}"
        >
          {SITE.email}
        </a>
      </div>
      <div class="flex flex-col gap-[3px]">
        <span class="text-xs text-fg-3">Text corrections</span>
        <a
          class="text-base text-fg transition-colors hover:text-accent"
          href="mailto:{SITE.correctionsEmail}"
        >
          {SITE.correctionsEmail}
        </a>
      </div>
      <div class="flex flex-col gap-[3px]">
        <span class="text-xs text-fg-3">Typical reply time</span>
        <span class="text-base text-fg">Two or three days</span>
      </div>
    </div>
  </div>

  <!-- Right: form card -->
  <Card class="rounded-xl p-[30px]">
    {#if sent}
      <div class="flex flex-col gap-2.5 px-1 py-6 animate-fade-up">
        <div
          class="flex h-[38px] w-[38px] items-center justify-center rounded-full bg-accent-soft text-accent"
        >
          <Icon name="check" size={17} />
        </div>
        <div class="text-lg font-semibold">Message sent</div>
        <p class="text-base leading-[1.65] text-fg-2">
          Jazakallahu khayran — we'll reply to you shortly.
        </p>
        <button
          type="button"
          onclick={reset}
          class="mt-1.5 w-fit text-[14.5px] text-accent underline underline-offset-2 transition-opacity hover:opacity-80"
        >
          Send another
        </button>
      </div>
    {:else}
      <form class="flex flex-col gap-4" onsubmit={(e) => { e.preventDefault(); send(); }}>
        <div class="flex flex-col gap-[7px]">
          <Label for="contact-name" class="text-fg-2">Your name</Label>
          <Input
            id="contact-name"
            bind:value={name}
            placeholder="Aisha"
            class="h-auto rounded-lg border-line bg-bg-3 px-3.5 py-3 text-[15px] text-fg placeholder:text-fg-4 md:text-[15px]"
          />
        </div>

        <div class="flex flex-col gap-[7px]">
          <Label for="contact-email" class="text-fg-2">Email</Label>
          <Input
            id="contact-email"
            type="email"
            bind:value={email}
            placeholder="you@example.com"
            class="h-auto rounded-lg border-line bg-bg-3 px-3.5 py-3 text-[15px] text-fg placeholder:text-fg-4 md:text-[15px]"
          />
        </div>

        <div class="flex flex-col gap-[7px]">
          <span class="text-sm text-fg-2">What's this about?</span>
          <div role="group" aria-label="Topic" class="flex flex-wrap gap-1.5">
            {#each CONTACT_TOPICS as t (t)}
              <button
                type="button"
                onclick={() => (topic = topic === t ? null : t)}
                aria-pressed={topic === t}
                class={cn(
                  "rounded-full border px-3.5 py-2 text-sm transition-colors",
                  topic === t
                    ? "border-accent bg-accent-soft text-accent"
                    : "border-line text-fg-2 hover:bg-bg-3",
                )}
              >
                {t}
              </button>
            {/each}
          </div>
        </div>

        <div class="flex flex-col gap-[7px]">
          <Label for="contact-message" class="text-fg-2">Message</Label>
          <Textarea
            id="contact-message"
            bind:value={message}
            rows={5}
            placeholder="Tell us as much as you like…"
            class="resize-y field-sizing-fixed rounded-lg border-line bg-bg-3 px-3.5 py-3 text-[15px] leading-[1.6] text-fg placeholder:text-fg-4 md:text-[15px]"
          ></Textarea>
        </div>

        <Button type="submit" variant="accent" size="lg" class="w-full">Send message</Button>
      </form>
    {/if}
  </Card>
</div>
