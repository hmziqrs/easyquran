<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { focusFirstInvalid } from "$lib/auth/components/auth-form-focus";
  import type { Snippet } from "svelte";

  type Props = {
    heading: string;
    subheading?: string;
    submitLabel: string;
    pending?: boolean;
    serverError?: string | null;
    successNotice?: string | null;
    onsubmit: () => void | Promise<void>;
    children?: Snippet;
    footer?: Snippet;
  };

  let {
    heading,
    subheading,
    submitLabel,
    pending = false,
    serverError = null,
    successNotice = null,
    onsubmit,
    children,
    footer,
  }: Props = $props();

  let formEl: HTMLFormElement | null = $state(null);

  async function handleSubmit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (pending) return;
    await onsubmit();
    if (!formEl) return;
    focusFirstInvalid(formEl);
  }
</script>

<form bind:this={formEl} onsubmit={handleSubmit} class="flex flex-col gap-5" novalidate>
  <div class="flex flex-col gap-2">
    <h1 class="text-[28px] font-semibold leading-tight tracking-[-0.02em]">{heading}</h1>
    {#if subheading}
      <p class="text-[15px] leading-relaxed text-fg-2">{subheading}</p>
    {/if}
  </div>

  {#if serverError}
    <p role="alert" aria-live="assertive" class="rounded-lg border border-destructive/30 bg-destructive/5 px-3.5 py-2.5 text-sm text-destructive">
      {serverError}
    </p>
  {:else if successNotice}
    <p role="status" aria-live="polite" class="rounded-lg border border-accent/30 bg-accent-soft px-3.5 py-2.5 text-sm text-accent">
      {successNotice}
    </p>
  {/if}

  <div class="flex flex-col gap-4">
    {#if children}{@render children()}{/if}
  </div>

  <Button type="submit" variant="accent" size="lg" disabled={pending} class="mt-1 w-full">
    {pending ? "Please wait…" : submitLabel}
  </Button>

  {#if footer}
    <div class="flex flex-col gap-1.5 text-center text-sm text-fg-2">
      {@render footer()}
    </div>
  {/if}
</form>
