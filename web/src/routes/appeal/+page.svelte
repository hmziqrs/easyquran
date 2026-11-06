<script lang="ts">
  import { goto } from "$app/navigation";
  import { appealTokenFromInput } from "#lib/appeal/route.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import {
    appeal_entry_help,
    appeal_entry_label,
    appeal_entry_submit,
    appeal_intro,
    appeal_invalid_link,
    appeal_page_title,
  } from "#lib/i18n/m/appeal.js";

  let entry = $state("");
  let invalid = $state(false);

  function openAppeal(event: SubmitEvent): void {
    event.preventDefault();
    const token = appealTokenFromInput(entry);
    if (token === null) {
      invalid = true;
      return;
    }
    invalid = false;
    // The token lives in the path, never in a query string.
    void goto(`/appeal/${token}`);
  }
</script>

<svelte:head>
  <title>{appeal_page_title()}</title>
  <meta name="robots" content="noindex" />
</svelte:head>

<main class="mx-auto flex w-full max-w-xl flex-col gap-6 px-4 py-10">
  <h1 class="text-2xl font-semibold text-foreground">{appeal_page_title()}</h1>
  <p class="text-sm text-muted-foreground">{appeal_intro()}</p>
  <form class="flex flex-col gap-3" onsubmit={openAppeal}>
    <Label for="appeal-entry">{appeal_entry_label()}</Label>
    <Input
      id="appeal-entry"
      name="appeal-entry"
      autocomplete="off"
      spellcheck={false}
      aria-describedby="appeal-entry-help"
      aria-invalid={invalid ? "true" : undefined}
      bind:value={entry}
    />
    <p id="appeal-entry-help" class="text-xs text-muted-foreground">{appeal_entry_help()}</p>
    {#if invalid}
      <p role="alert" class="text-sm text-danger">{appeal_invalid_link()}</p>
    {/if}
    <Button type="submit" class="self-start">{appeal_entry_submit()}</Button>
  </form>
</main>
